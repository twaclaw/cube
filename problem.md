Create a Rust application based on trunk, as light as possible to be served on Github pages.

# Description

This app is a visualization of a rubik's cube using similar visualization to the ones in the notebooks in ../cube_groups/kociemba.ipynb.

Reuse existing libraries and visualization if they exist, start by checking the web.

We will have two visualizations of the cube (in parallel, one next to the other)

- A 3d representation that can be rotated like the one provided in the notebook. The cube can be rotated (zoom disabled by default)
- A 2d open representation of the cube (the open cube) with a checkbox to add the facelets numbers to the facelets. 

# Features

- Both representation use the standard cube colors.
- A text entry where a sequences of moves can be input in the same format used in the notebooks, for instance "rfu". When pressing enter, the sequences should be applied.
- When the sequence is applied, there are buttons to either apply the sequence instantly, a stepper to do step by step, or a play button to play the sequence.
- The moves should be applied slowly and there should be an animation of the rotation of the faces. 
- There should be a scramble button than sets the cube (instantly) to a random state.
- Another button sets the cube to the solved state.
- Create another button that in the future will solve the cube (don't implement now, the solver will be implemetned later)
- Images and sequences should be exportable in high quality. The idea of the exports is to embed them in a markdown blog (astro) either as svg or as html animations.
- Have a look at the operations performed in this work: http://www.geometer.org/rubik/, it has a document describing the software;. Basically, it has fields to enter texts describing macros face moves, slice moves, whole cube moves, etc. We don't need to implement it all now but keep it into account in the original design.
- The graphics have to be very high quality.  being able to rotate the cube before the animations or to define pre-defined camera views.

- Add the ability to highlight some facelets in the 2d diagram in the way it is done in the notebook, for instance to emphasize that moves in the Group H are invariant for some cubies. 
- Initially it is for a standard 3x3x3 cube, and the solver will be only for this cube, but try to keep the representations general enough so that larger or smaller cubes can also be drawn in the future.
- Keep the code comments concise, use idiomatic rust, don't hardcode rust versions, use cargo update, etc. 

