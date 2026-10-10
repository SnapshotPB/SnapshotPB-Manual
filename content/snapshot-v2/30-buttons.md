# The buttons and the screen

The board has four buttons: UP, MENU, DOWN and LOCKOUT. A press is a short push on a button. A hold is a push of {{value define.MENU_LONGPRESS_MS}} ms or more.

After the start, the screen shows the boot text for a moment. Then it shows the home screen. The home screen shows the shot counter.

## Turn the board off and on

1. Hold MENU. The screen shows `Powering Off`, and then the screen goes dark.
2. To start the board again, hold MENU.

NOTE: A press of MENU wakes the board, and the board goes back to sleep if you release MENU before the hold time.

## What each button does

The tables below come from the state machine of the firmware. Each table is one screen of the board.

{{buttons}}

## The captions

The screen shows a caption for a moment after some actions.

{{captions}}
