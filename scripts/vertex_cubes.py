#!/usr/bin/env python3
"""Generate scenes/vertex_cubes.toml and configs/image/vertex_cubes.toml.

Seen down the (1, 1, 1) diagonal a cube is corner on and its silhouette is a
regular hexagon. A neighbour is placed by stepping along one of the three
visible faces' axes and back along the other two, which leaves GAP between the
two facing faces; the three steps are permutations of each other, so neighbours
sit 120 degrees apart around a cube.

Taken literally that step also creeps towards the camera, so a field of them
would drift forwards rather than share a plane. Each step is therefore flattened
onto the plane through the origin normal to (1, 1, 1), which leaves the
silhouettes where they are and puts every cube on one plane. Flattening eats
part of the gap, so the step is sized to give GAP back once flattened.

The camera is then tilted off that normal, so the plane is seen at an angle and
recedes up the frame instead of sitting dead on. Cubes are placed wherever the
lattice lands inside the tilted frustum, and coloured by a swirl of hue about a
point on the plane: hue follows the angle about that point and drifts with the
distance from it, so it covers every hue smoothly without reading as a ramp.

The script writes the config as well as the scene because the two have to agree:
the camera decides which lattice cells are in frame.
"""

import argparse
import math
from pathlib import Path

# What this design is called: it names the scene, the config and the renders, so
# a new design is a copy of this script with a new NAME rather than an edit here.
NAME = "vertex_cubes"

SIDE = 1.0
GAP = 0.2

# Variants are the same design at another size or spacing, so they come in on
# the command line and write under their own name rather than forking the file.
_args = argparse.ArgumentParser(description=__doc__.splitlines()[0])
_args.add_argument("--name", default=NAME)
_args.add_argument("--side", type=float, default=SIDE)
_args.add_argument("--gap", type=float, default=GAP)
_opts = _args.parse_args()
NAME, SIDE, GAP = _opts.name, _opts.side, _opts.gap

# Camera. The tilts swing it off the plane's normal, in degrees, along the
# frame's up and right axes.
DISTANCE = 20.0
FOV = 30.0
TILT_UP = -28.0
TILT_RIGHT = 27.0
WIDTH = 420
HEIGHT = 594
SAMPLES = 64
BOUNCES = 8
# A shallow depth of field, focused on the middle of the frame so the near
# bottom right and the far top left both soften a little.
DEFOCUS_ANGLE = 2.2

# Colour. Cubes are coloured in Oklab, where equal steps are equally different
# to the eye, so no pair of neighbours stands out as a jump while another pair
# looks the same. The field's two axes drive Oklab's two chroma axes and its
# lightness, which varies hue, saturation and brightness together rather than
# running a hue around a wheel. WARP bends the axes first so the result reads as
# neither a linear ramp nor a radial one.
WARP = 0.28
CHROMA = 0.13
# Where the field sits in the chroma plane, and how it is turned within it. The
# frame is tall, so its long axis carries most of the picture: turning that axis
# onto the green-to-orange diagonal and nudging the centre towards yellow keeps
# the blues and violets from taking over.
CHROMA_CENTRE = (0.005, 0.03)
CHROMA_ANGLE = -40.0
LIGHTNESS = 0.68
LIGHTNESS_SWING = 0.09

# A quad light behind the camera, up and to one side so the cubes shadow each
# other. Placed by direction from the origin, size and radiance.
LIGHT_UP = 1.0
LIGHT_RIGHT = -0.8
LIGHT_DISTANCE = 26.0
LIGHT_SIZE = 9.0
LIGHT_RADIANCE = 50.0

HALF = SIDE / 2
# Flattening removes a third of the step's overshoot from each axis, so 1.5 * GAP
# of overshoot leaves exactly GAP between faces afterwards.
STEP = SIDE + 1.5 * GAP
# Circumradius of a cube, the furthest any of its corners reaches.
REACH = SIDE * math.sqrt(3) / 2

ASPECT = WIDTH / HEIGHT
VUP = (0.0, 1.0, 0.0)

NORMAL = (1 / math.sqrt(3), 1 / math.sqrt(3), 1 / math.sqrt(3))
RIGHT = (1 / math.sqrt(2), 0.0, -1 / math.sqrt(2))
UP = (-1 / math.sqrt(6), 2 / math.sqrt(6), -1 / math.sqrt(6))


def dot(a, b):
    return sum(x * y for x, y in zip(a, b))


def add(*vs):
    return tuple(sum(v[k] for v in vs) for k in range(3))


def sub(a, b):
    return tuple(x - y for x, y in zip(a, b))


def scale(v, s):
    return tuple(x * s for x in v)


def cross(a, b):
    return (
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    )


def unit(v):
    return scale(v, 1 / math.sqrt(dot(v, v)))


def flatten(v):
    """Drop the component along the plane's normal."""
    return add(v, scale(NORMAL, -dot(v, NORMAL)))


# One step per visible face: +x (right) and +y (above). The third, +z, is the
# negative of their sum once flattened, so it needs no separate generator.
A = flatten((STEP, -HALF, -HALF))
B = flatten((-HALF, STEP, -HALF))

POSITION = scale(
    unit(
        add(
            NORMAL,
            scale(UP, math.tan(math.radians(TILT_UP))),
            scale(RIGHT, math.tan(math.radians(TILT_RIGHT))),
        )
    ),
    DISTANCE,
)
LOOK_AT = (0.0, 0.0, 0.0)

# Camera basis: back, right, up.
CAM_W = unit(sub(POSITION, LOOK_AT))
CAM_U = unit(cross(VUP, CAM_W))
CAM_V = cross(CAM_W, CAM_U)
TAN_HALF_FOV = math.tan(math.radians(FOV) / 2)


def centre(i, j):
    return add(scale(A, i), scale(B, j))


def plane_coords(c):
    """Where a point sits on the plane, in its own right and up axes."""
    return dot(c, RIGHT), dot(c, UP)


def visible(c):
    """Whether a cube at this centre shows any of itself in frame."""
    d = sub(c, POSITION)
    depth = -dot(d, CAM_W)
    if depth <= REACH:
        return False
    half_h = depth * TAN_HALF_FOV
    slack = REACH
    return (
        abs(dot(d, CAM_U)) <= half_h * ASPECT + slack and abs(dot(d, CAM_V)) <= half_h + slack
    )


def vec(v):
    return "[" + ", ".join(f"{x:.4g}" for x in v) + "]"


def light():
    d = unit(add(NORMAL, scale(UP, LIGHT_UP), scale(RIGHT, LIGHT_RIGHT)))
    pos = scale(d, LIGHT_DISTANCE)
    # Edges spanning the quad, ordered so cross(u, v) points back at the scene:
    # a diffuse light only emits from the side its normal faces.
    u = scale(unit(cross(d, UP)), LIGHT_SIZE)
    v = scale(unit(cross(d, u)), LIGHT_SIZE)
    if dot(cross(u, v), d) > 0:
        u, v = v, u
    corner = add(pos, scale(u, -0.5), scale(v, -0.5))
    return (
        f'[materials.lamp]\ntype = "diffuse_light"\n'
        f"color = [{LIGHT_RADIANCE:.4g}, {LIGHT_RADIANCE:.4g}, {LIGHT_RADIANCE:.4g}]\n\n"
        f'[[objects]]\nname = "key light"\ntype = "quad"\nmaterial = "lamp"\n'
        f"corner = {vec(corner)}\nu = {vec(u)}\nv = {vec(v)}\n"
    )


def oklab_to_rgb(lightness, a, b):
    """Oklab to linear RGB, which is what an albedo is."""
    l = (lightness + 0.3963377774 * a + 0.2158037573 * b) ** 3
    m = (lightness - 0.1055613458 * a - 0.0638541728 * b) ** 3
    s = (lightness - 0.0894841775 * a - 1.2914855480 * b) ** 3
    return (
        4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
        -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
        -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s,
    )


def in_gamut(rgb):
    return all(-1e-6 <= x <= 1.0 for x in rgb)


def albedo(lightness, a, b):
    """The colour, with chroma pulled in until it is a colour that exists."""
    if in_gamut(oklab_to_rgb(lightness, a, b)):
        return oklab_to_rgb(lightness, a, b)
    lo, hi = 0.0, 1.0
    for _ in range(24):
        mid = (lo + hi) / 2
        if in_gamut(oklab_to_rgb(lightness, a * mid, b * mid)):
            lo = mid
        else:
            hi = mid
    return tuple(max(x, 0.0) for x in oklab_to_rgb(lightness, a * lo, b * lo))


def colour_at(c, extent):
    """Where this cube sits in Oklab, from where it sits on the plane.

    The two axes are normalised separately: the field is taller than it is wide,
    and sharing one scale would leave most of one chroma axis unused.
    """
    x, y = plane_coords(c)
    u, v = x / extent[0], y / extent[1]
    # Bend the axes into each other so neither one reads as a straight ramp.
    u, v = u + WARP * math.sin(2.1 * v + 0.6), v + WARP * math.sin(1.7 * u - 1.1)
    lightness = LIGHTNESS + LIGHTNESS_SWING * math.sin(1.4 * u - 1.0 * v + 0.3)
    turn = math.radians(CHROMA_ANGLE)
    a = u * math.cos(turn) - v * math.sin(turn)
    b = u * math.sin(turn) + v * math.cos(turn)
    return lightness, CHROMA_CENTRE[0] + CHROMA * a, CHROMA_CENTRE[1] + CHROMA * b


def material(i, j, c, extent, taken):
    """A cube's colour, nudged if the field gave two cubes the same one.

    Lightness is walked in steps too small to see until the colour is its own.
    """
    lightness, a, b = colour_at(c, extent)
    while True:
        rgb = albedo(lightness, a, b)
        colour = "[" + ", ".join(f"{x:.3f}" for x in rgb) + "]"
        if colour not in taken:
            break
        lightness -= 0.002
    taken.add(colour)
    return f'[materials."cube {i},{j}"]\ntype = "lambertian"\ncolor = {colour}\n'


def cube(i, j, c):
    lo = [v - HALF for v in c]
    hi = [v + HALF for v in c]
    return (
        f'[[objects]]\nname = "cube {i},{j}"\ntype = "box"\n'
        f'material = "cube {i},{j}"\nmin = {vec(lo)}\nmax = {vec(hi)}\n'
    )


def config():
    return (
        f"# Generated by scripts/{NAME}.py - edit that, not this.\n\n"
        'type = "image"\n\n'
        f"[camera]\nposition = {vec(POSITION)}\nlook_at = {vec(LOOK_AT)}\n"
        f"vup = {vec(VUP)}\nfov = {FOV:.4g}\ndefocus_angle = {DEFOCUS_ANGLE:.4g}\n"
        f"focus_dist = {DISTANCE:.4g}\n\n"
        f"[quality]\nsamples = {SAMPLES}\nbounces = {BOUNCES}\n\n"
        f"[output]\nwidth = {WIDTH}\nheight = {HEIGHT}\n"
    )


def main():
    # Wide enough that the tilted frustum's far edge is inside the search.
    span = int(4 * DISTANCE / math.sqrt(dot(A, A))) + 2
    placed = [
        (i, j, centre(i, j))
        for i in range(-span, span + 1)
        for j in range(-span, span + 1)
        if visible(centre(i, j))
    ]
    extent = tuple(
        max(abs(plane_coords(c)[k]) for _, _, c in placed) for k in (0, 1)
    )
    taken = set()
    parts = [
        f"# Generated by scripts/{NAME}.py - edit that, not this.\n",
        "background = [0.0, 0.0, 0.0]\n",
        light(),
        *(material(*p, extent, taken) for p in placed),
        *(cube(*p) for p in placed),
    ]
    root = Path(__file__).resolve().parent.parent
    (root / "scenes" / f"{NAME}.toml").write_text("\n".join(parts))
    (root / "configs" / "image" / f"{NAME}.toml").write_text(config())
    print(f"{len(placed)} cubes, side {SIDE}, gap {GAP}")


if __name__ == "__main__":
    main()
