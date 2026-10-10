# The board

The {{value product}} Autococker {{value board.name}} board controls the firing of a paintball marker. The board drives the two solenoids of the marker, reads the trigger, reads a ball eye, and shows a menu on a small screen. A mobile app configures the board through Bluetooth.

{{image pcb:snapshot_v2/renders/03-iso-top.png | The main board, with the screen board and the trigger board in place}}

## The parts that you use

| Part | Where | What it does |
|---|---|---|
| Screen | The screen board, behind the window of the grip frame | Shows the shot counter, the menu and the captions |
| UP, MENU and DOWN buttons | The back of the grip frame, in the three button holes | Move in the menu and change a setting |
| LOCKOUT button | On the board | Locks and unlocks the configuration |
| Status LED | On the board | Shows the state of the board |
| `BATT` | A 2-pin connector | The battery input |
| `SEAR` | A 2-pin connector | The sear solenoid output |
| `COCKING` | A 2-pin connector | The cocking solenoid output |
| `EYES` | A 4-pin connector | The ball eye, which is off the board |
| `TRIG` | A 4-pin header | The trigger board, which is an optical board or a switch board |
| USB-C | The USB-C receptacle | Data and programming. USB-C does not give power to the board. |

{{image pcb:docs/images/quick-start/buttons-back.svg | The three buttons at the back of the grip frame}}

## The status LED

| Colour | Meaning |
|---|---|
| Green | The board is on |
| Blue | The Bluetooth radio is on |
| Red | The board found a fault at start. The LED stays red until the next start. |
