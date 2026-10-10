# Install the board

WARNING: Remove the air source from the marker before you touch the board. A solenoid can fire the marker when the board gets power.

CAUTION: Touch a grounded metal object before you touch the board. Static electricity can damage the parts.

## Install the trigger board

The trigger board is an optical board or a switch board. Only one trigger board fits at a time. The board goes onto the `TRIG` header, and one screw holds it.

{{image pcb:docs/images/quick-start/trigger-board-install.svg | The trigger board above the main board, before the installation}}

1. Put the spacer on hole `H3` of the main board.
2. Hold the trigger board above the main board, with socket `J11` above header `TRIG1`.
3. Push the trigger board down, until the socket is fully on the header.
4. Put the M2.5 x 16 screw through hole `H4` of the trigger board and through the spacer.
5. Tighten the screw.

{{image pcb:docs/images/quick-start/trigger-board-seated.svg | The trigger board in place}}

## Connect the solenoids

1. Connect the sear solenoid to `SEAR`.
2. Connect the cocking solenoid to `COCKING`.

NOTE: Pin 2 of each connector is the coil supply, and pin 1 is the switched side.

## Connect the ball eye

Connect the eye to `EYES`. The eye is optional. A board with no eye fires on the timing alone.

## Connect the battery

CAUTION: Make sure that the polarity of the battery lead is correct. Pin 1 of `BATT` is the negative pin. Pad 1 is the rectangular pad, and a dot on the silk marks it.

1. Select the type of battery: one 9 V cell, or one 2S LiPo pack.
2. Connect the battery lead to `BATT`. The board starts.
3. Set the Battery row of the Setup screen to the type of your battery. See [Setup](#screen-setup-menu).

NOTE: The board uses the battery type to select the voltage at which it shuts down. A wrong type can discharge a LiPo pack too far.
