"""Compare a composited capture with reference rasters placed on the device grid.

Each window is an opaque buffer of its background colour. Each of its layers is
a straight-alpha RGBA8 reference raster at a whole device-pixel position in that
buffer, composited source-over in 8-bit sRGB in order. A pixel that a layer
covers completely takes that layer's colour exactly; a pixel that it leaves
clear keeps the colour below. A partially covered pixel is the exactly rounded
blend (c * a + d * (255 - a)) / 255. A renderer that premultiplies and scales
the destination with its own rounding differs from that blend by at most one
8-bit step, and by at most one more step for every further partially covering
layer below, so the allowed difference of a pixel is the number of layers that
partially cover it: 0 where every layer covers it completely or not at all.

The compositor places each window buffer on the capture over the backdrop
colour. When the destination box it computes has the buffer's size, every
buffer pixel lands on one capture pixel and the bounds above hold unchanged.
Otherwise the compositor resamples the buffer, and each capture pixel is only
bracketed: every channel must lie within the bounds of the buffer pixels that
its sample position falls between. Where it falls beyond the buffer's edge, the
transparent sample may darken the pixel towards black, because a compositor
may copy an opaque buffer without blending it over the colour below.

Native text is not a reference raster. Each text region, the device pixels that
its published bounds touch, is excluded from that comparison. Every channel of
a pixel there must instead lie between the bounds below the text and the text's
foreground colour, widened by one step for the text's own blend.
"""
import math
import struct

from PIL import Image, ImageChops


class Layer:
    def __init__(self, name, x, y, image):
        if image.mode != "RGBA":
            raise ValueError(f"the {name} reference raster is not RGBA8")
        self.name, self.x, self.y, self.image = name, x, y, image

    @property
    def rectangle(self):
        return (self.x, self.y, self.x + self.image.width, self.y + self.image.height)


class TextRegion:
    def __init__(self, name, rectangle, foreground):
        self.name, self.rectangle, self.foreground = name, rectangle, tuple(foreground)


def f32(value):
    return struct.unpack("f", struct.pack("f", value))[0]


def c_round(value):
    return int(math.copysign(math.floor(abs(value) + .5), value))


def whole(value, description):
    if type(value) not in (int, float) or value != int(value):
        raise ValueError(f"{description} {value!r} is not a whole logical pixel")
    return int(value)


def compositor_origin(value, scale, description):
    """The output device pixel at which Sway places a surface's whole logical coordinate.

    wlroots 0.20 scene rendering (types/scene/wlr_scene.c, scale_box) scales an
    integer logical box edge by the single-precision output scale and rounds it
    half away from zero, so a fractional device edge such as 123.75 lands on 124.
    """
    return c_round(f32(f32(whole(value, description)) * f32(scale)))


def compositor_length(length, offset, scale, description):
    """The device extent of a logical box in the wlroots 0.20 scene: its rounded far edge minus its near edge."""
    return compositor_origin(whole(offset, description) + whole(length, description), scale, description) \
        - compositor_origin(offset, scale, description)


def buffer_length(length, scale, description):
    """The device extent of a Qt Wayland window buffer: the logical size times the scale, rounded by qRound."""
    return math.floor(whole(length, description) * scale + .5)


class Window:
    """A window buffer of its background colour with reference layers and text regions in buffer pixels."""

    def __init__(self, name, bounds, scale, background, layers=(), texts=()):
        self.name, self.background = name, tuple(background)
        self.layers, self.texts = list(layers), list(texts)
        description = f"the {name} window's logical bounds"
        self.origin = tuple(compositor_origin(bounds[axis], scale, description) for axis in ("x", "y"))
        self.destination = tuple(compositor_length(bounds[extent], bounds[axis], scale, description)
                                 for axis, extent in (("x", "width"), ("y", "height")))
        self.buffer = tuple(buffer_length(bounds[extent], scale, description) for extent in ("width", "height"))

    @property
    def rectangle(self):
        return (*self.origin, self.origin[0] + self.destination[0], self.origin[1] + self.destination[1])

    @property
    def resampled(self):
        return self.destination != self.buffer

    def summary(self):
        return {"origin": list(self.origin), "destination": list(self.destination), "buffer": list(self.buffer),
                "footprints": {layer.name: list(layer.rectangle) for layer in self.layers},
                "texts": {text.name: list(text.rectangle) for text in self.texts}}


def text_rectangle(bounds, scale):
    """The device pixels touched by logical text bounds in a window buffer."""
    return (math.floor(bounds["x"] * scale), math.floor(bounds["y"] * scale),
            math.ceil((bounds["x"] + bounds["width"]) * scale), math.ceil((bounds["y"] + bounds["height"]) * scale))


def placed_viewport(bounds, placement, scale):
    """Round paint bounds outward on the device grid after placement, as the CPU realization prepares them.

    Returns the device rectangle relative to the window and the logical viewport origin
    relative to the placement that the reference raster samples from.
    """
    left = math.floor((bounds["x"] + placement[0]) * scale)
    top = math.floor((bounds["y"] + placement[1]) * scale)
    right = math.ceil((bounds["x"] + bounds["width"] + placement[0]) * scale)
    bottom = math.ceil((bounds["y"] + bounds["height"] + placement[1]) * scale)
    return (left, top, right - left, bottom - top), (left / scale - placement[0], top / scale - placement[1])


def contains(outer, inner):
    return outer[0] <= inner[0] and outer[1] <= inner[1] and inner[2] <= outer[2] and inner[3] <= outer[3]


def compose(size, background, layers):
    """Composite the layers over a background; return the expected RGB image and each pixel's allowed difference."""
    expected = Image.new("RGB", size, tuple(background))
    tolerance = Image.new("L", size, 0)
    for layer in layers:
        if not contains((0, 0, *size), layer.rectangle):
            raise ValueError(f"the {layer.name} reference raster {layer.rectangle} leaves the {size[0]}x{size[1]} "
                             "buffer")
        below = expected.crop(layer.rectangle).tobytes()
        allowed = bytearray(tolerance.crop(layer.rectangle).tobytes())
        source = layer.image.tobytes()
        result = bytearray(below)
        for index in range(len(allowed)):
            alpha = source[4 * index + 3]
            if alpha == 0:
                continue
            if alpha == 255:
                result[3 * index:3 * index + 3] = source[4 * index:4 * index + 3]
                allowed[index] = 0
                continue
            for channel in range(3):
                total = source[4 * index + channel] * alpha + below[3 * index + channel] * (255 - alpha)
                result[3 * index + channel] = (2 * total + 255) // 510
            allowed[index] = min(255, allowed[index] + 1)
        expected.paste(Image.frombytes("RGB", layer.image.size, bytes(result)), layer.rectangle[:2])
        tolerance.paste(Image.frombytes("L", layer.image.size, bytes(allowed)), layer.rectangle[:2])
    return expected, tolerance


def sources(destination, buffer):
    """For each destination pixel on one axis, the buffer pixels its centre's sample position falls between.

    On an axis that is not scaled each pixel samples exactly its own buffer pixel. -1 or the
    buffer length stands for a transparent sample beyond the buffer's edge. A margin of 1/256
    pixel covers the compositor's fixed-point sample positions.
    """
    if destination == buffer:
        return [range(index, index + 1) for index in range(buffer)]
    ratio = buffer / destination
    result = []
    for index in range(destination):
        position = (index + .5) * ratio - .5
        result.append(range(max(-1, math.floor(position - 1 / 256)), min(buffer + 1, math.floor(position + 1 / 256) + 2)))
    return result


def resampled_bounds(window, expected, tolerance, below_low, below_high):
    """Bracket each destination pixel of a resampled window by the buffer pixels it can sample."""
    width, height = window.buffer
    low = ImageChops.subtract(expected, Image.merge("RGB", [tolerance] * 3)).tobytes()
    high = ImageChops.add(expected, Image.merge("RGB", [tolerance] * 3)).tobytes()
    below_low, below_high = below_low.tobytes(), below_high.tobytes()
    columns, rows = sources(window.destination[0], width), sources(window.destination[1], height)
    out_low, out_high = bytearray(len(below_low)), bytearray(len(below_high))
    for y, row in enumerate(rows):
        for x, column in enumerate(columns):
            target = 3 * (y * window.destination[0] + x)
            for channel in range(3):
                values_low, values_high = [], []
                for source_y in row:
                    for source_x in column:
                        if 0 <= source_x < width and 0 <= source_y < height:
                            index = 3 * (source_y * width + source_x) + channel
                            values_low.append(low[index])
                            values_high.append(high[index])
                        else:
                            # Beyond the buffer the sample is transparent: blended over the colour below, or
                            # transparent black where the compositor copies an opaque buffer without blending.
                            values_low += [0, below_low[target + channel]]
                            values_high.append(below_high[target + channel])
                out_low[target + channel], out_high[target + channel] = min(values_low), max(values_high)
    return (Image.frombytes("RGB", window.destination, bytes(out_low)),
            Image.frombytes("RGB", window.destination, bytes(out_high)))


def destination_rectangle(window, rectangle):
    """A buffer rectangle in capture pixels, widened by one pixel on each side where the window is resampled."""
    if not window.resampled:
        return (window.origin[0] + rectangle[0], window.origin[1] + rectangle[1],
                window.origin[0] + rectangle[2], window.origin[1] + rectangle[3])
    sx, sy = (window.destination[axis] / window.buffer[axis] for axis in (0, 1))
    return (window.origin[0] + max(0, math.floor(rectangle[0] * sx) - 1),
            window.origin[1] + max(0, math.floor(rectangle[1] * sy) - 1),
            window.origin[0] + min(window.destination[0], math.ceil(rectangle[2] * sx) + 1),
            window.origin[1] + min(window.destination[1], math.ceil(rectangle[3] * sy) + 1))


def expectation(size, backdrop, windows):
    """The nominal expected capture and the per-pixel lower and upper bounds of every channel."""
    capture = (0, 0, *size)
    nominal = Image.new("RGB", size, tuple(backdrop))
    low, high = nominal.copy(), nominal.copy()
    for window in windows:
        if not contains(capture, window.rectangle):
            raise ValueError(f"the {window.name} window {window.rectangle} leaves the {size[0]}x{size[1]} capture")
        expected, tolerance = compose(window.buffer, window.background, window.layers)
        if window.resampled:
            bounds = resampled_bounds(window, expected, tolerance, low.crop(window.rectangle),
                                      high.crop(window.rectangle))
            nominal.paste(expected.resize(window.destination, Image.Resampling.BILINEAR), window.origin)
        else:
            spread = Image.merge("RGB", [tolerance] * 3)
            bounds = ImageChops.subtract(expected, spread), ImageChops.add(expected, spread)
            nominal.paste(expected, window.origin)
        low.paste(bounds[0], window.origin)
        high.paste(bounds[1], window.origin)
    return nominal, low, high


def owner(point, windows):
    for window in reversed(windows):
        if contains(window.rectangle, (*point, point[0] + 1, point[1] + 1)):
            if window.resampled:
                return f"the resampled {window.name} window"
            local = (point[0] - window.origin[0], point[1] - window.origin[1], point[0] - window.origin[0] + 1,
                     point[1] - window.origin[1] + 1)
            names = [layer.name for layer in window.layers if contains(layer.rectangle, local)]
            return f"the {names[-1]} footprint" if names else f"the {window.name} window background"
    return "the backdrop"


def channel_max(image):
    bands = image.split()
    return ImageChops.lighter(ImageChops.lighter(bands[0], bands[1]), bands[2])


def compare(actual, backdrop, windows):
    """Compare a capture with its expected composition; raise on the first differences, else summarize."""
    actual = actual.convert("RGB")
    nominal, low, high = expectation(actual.size, backdrop, windows)
    capture = (0, 0, *actual.size)
    mask = Image.new("L", actual.size, 0)
    failures = []
    for window in windows:
        for text in window.texts:
            if not contains((0, 0, *window.buffer), text.rectangle) or text.rectangle[2] <= text.rectangle[0] \
                    or text.rectangle[3] <= text.rectangle[1]:
                raise ValueError(f"the {text.name} text region {text.rectangle} is empty or leaves its window")
            region = destination_rectangle(window, text.rectangle)
            mask.paste(255, region)
            width = region[2] - region[0]
            lows, highs = low.crop(region).tobytes(), high.crop(region).tobytes()
            pixels = actual.crop(region).tobytes()
            for index in range(len(pixels) // 3):
                span = [(min(lows[3 * index + c], text.foreground[c]) - 1,
                         max(highs[3 * index + c], text.foreground[c]) + 1) for c in range(3)]
                if any(not lower <= pixels[3 * index + c] <= upper for c, (lower, upper) in enumerate(span)):
                    point = (region[0] + index % width, region[1] + index // width)
                    failures.append(f"{point} in the {text.name} text region is {actual.getpixel(point)}, not within "
                                    f"{[list(bound) for bound in span]} between the colour below and the foreground "
                                    f"{text.foreground}")
                    if len(failures) >= 5:
                        break
    excess = ImageChops.lighter(channel_max(ImageChops.subtract(low, actual)),
                                channel_max(ImageChops.subtract(actual, high)))
    excess.paste(0, mask=mask)
    region = excess.getbbox()
    if region:
        crop = excess.crop(region).tobytes()
        width = region[2] - region[0]
        for index in (index for index, value in enumerate(crop) if value):
            point = (region[0] + index % width, region[1] + index // width)
            failures.append(f"{point} in {owner(point, windows)} is {actual.getpixel(point)}, not within "
                            f"{low.getpixel(point)} to {high.getpixel(point)}")
            if len(failures) >= 10:
                break
    if failures:
        raise ValueError("composited capture differs from the reference composition: " + "; ".join(failures))
    unmasked = ImageChops.invert(mask)
    aligned = unmasked.copy()
    for window in windows:
        if window.resampled:
            aligned.paste(0, window.rectangle)
    bounded = channel_max(ImageChops.subtract(high, low))
    difference = channel_max(ImageChops.difference(nominal, actual)).histogram(mask=aligned)
    return {"pixels": unmasked.histogram()[255], "masked": mask.histogram()[255],
            "partiallyCovered": sum(bounded.histogram(mask=aligned)[1:]),
            "resampledPixels": unmasked.histogram()[255] - aligned.histogram()[255],
            "differing": sum(difference[1:]),
            "maxDifference": max(value for value, count in enumerate(difference) if count or not value),
            "resampled": [window.name for window in windows if window.resampled],
            "windows": {window.name: window.summary() for window in windows}}, nominal
