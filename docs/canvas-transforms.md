# Canvas transforms

The Canvas menu provides integer-pixel crop bounds, nearest-neighbour image resize,
transparent canvas expansion with an explicit image offset, quarter-turn rotations,
and horizontal/vertical flips. Dimensions and offsets are in source image pixels,
independent of viewport zoom. A successful operation fits the result in the viewport.
Invalid bounds leave the previous image on screen and report an error. Opening or
transforming an image never writes to its source file.

Desktop transformations call `scribblecap_core::canvas::transform` through a native
worker. Core accepts straight RGBA8, checks input/output buffers against the 256 MiB
render budget before allocation, and returns new pixels without changing its input.
Crop and expansion must fit their respective source and destination bounds.
Quarter turns and flips preserve every channel exactly, including transparent RGB.
Resize uses nearest-neighbour sampling, preserving screenshot edges and avoiding
alpha interpolation fringes. The browser development preview uses Canvas 2D.

This implements raster-only transforms within the editor. Controls are disabled
when annotations are present or imported editable metadata exists. M2-7 remains open for
integration with editable annotations and M2-6 history: annotations must remain
editable and undo must retain the previous resource. The current document schema
cannot represent rotated/reflected text or badge artwork; that needs an explicit
format decision before implementing document-wide transformations. Images with embedded editable annotations are blocked in both the controls and native
transform command. No annotation flattening or schema extension is introduced here.

Validation: `cargo test -p scribblecap-core --test canvas --locked` covers exact
pixel positions, inverse operations, alpha-channel preservation, invalid buffers,
zero dimensions, overflowing bounds, and allocation limits. UI bridge tests cover
the WebKit byte-array transport and native argument mapping.
