# Fire modes and timing

## The fire modes

The Mode row of the Fire screen selects the fire mode.

{{enum AutocockerFireMode}}

## The firing cycle

The Timing screen sets the times of one firing cycle. All times are in milliseconds.

1. You pull the trigger. The trigger must stay pulled for the Debounce on time.
2. The board energizes the sear solenoid for the Sear dwell time. The marker fires.
3. The board waits for the Cock delay time.
4. The board energizes the cocking solenoid, which opens the breech. With no eye, the solenoid stays energized for the Cock open time. With an eye, the solenoid stays energized until the eye sees a ball, or until the Cock timeout time ends.
5. The board releases the cocking solenoid, and it waits for the Cock close time before the next cycle.

The BPS cap row of the Fire screen limits the rate of fire. The board fires at the slower of the cycle time and the BPS cap.

## The ball eye

The Eye mode row of the Eye screen selects the type of eye. The eye applies only in a fire mode with a cocking phase: SEMI, FULL AUTO and NXL 2026.

{{enum EyeMode}}

Hold UP on the home screen to turn the eye off or on for this session. The screen shows `EYES OFF` or `EYES ON`. The setting does not change, and the next start uses the setting.

## Ramping

The NXL 2026 mode applies the rules of the NXL 2026 season. The board fires at a maximum of {{value define.FIRE_CONTROL_DEFAULT_RATE_CAP_TENTHS_BPS}} tenths of a ball per second, which is 10.5 bps. The board enters the ramp after {{value kconfig.FIRE_CONTROL_RAMPING_NXL_2026_MIN_TRIGGER_PRESSES}} trigger pulls at an interval of {{value kconfig.FIRE_CONTROL_RAMPING_NXL_2026_MIN_TRIGGER_RATE_MS}} ms or less.

The Ramping screen holds the custom ramp settings. The board shows the Ramping row of the Fire screen only while the fire mode is NXL 2026.

## The battery

The Battery row of the Setup screen tells the board which battery you use.

{{enum BatteryType}}
