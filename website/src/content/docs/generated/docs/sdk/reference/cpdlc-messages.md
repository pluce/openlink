---
title: CPDLC Messages
description: Mirrored documentation from docs/sdk/reference/cpdlc-messages.md
sidebar:
  order: 19
---

> Source: docs/sdk/reference/cpdlc-messages.md (synced automatically)

# CPDLC Message Reference

Generated from catalog `cpdlc-catalog.v1` at `2026-02-20T09:05:49.110131+00:00`.

## Uplink messages (UM)

| ID | Template | Args | Resp | Closing | Standby | Constrained replies | FANS | ATN B1 |
|---|---|---|---|---|---|---|---|---|
| UM0 | UNABLE | - | N | Yes | No | - | Yes | Yes |
| UM1 | STANDBY | - | N | No | Yes | - | Yes | Yes |
| UM2 | REQUEST DEFERRED | - | N | No | Yes | - | Yes | No |
| UM3 | ROGER | - | N | Yes | No | - | Yes | Yes |
| UM4 | AFFIRM | - | N | Yes | No | - | Yes | Yes |
| UM5 | NEGATIVE | - | N | Yes | No | - | Yes | Yes |
| UM19 | MAINTAIN [level] | Level | WU | No | No | - | Yes | Yes |
| UM20 | CLIMB TO [level] | Level | WU | No | No | - | Yes | Yes |
| UM21 | AT [time] CLIMB TO [level] | Time, Level | WU | No | No | - | Yes | No |
| UM22 | AT [position] CLIMB TO [level] | Position, Level | WU | No | No | - | Yes | No |
| UM23 | DESCEND TO [level] | Level | WU | No | No | - | Yes | Yes |
| UM24 | AT [time] DESCEND TO [level] | Time, Level | WU | No | No | - | Yes | No |
| UM25 | AT [position] DESCEND TO [level] | Position, Level | WU | No | No | - | Yes | No |
| UM26 | CLIMB TO REACH [level] BY [time] | Level, Time | WU | No | No | - | Yes | Yes |
| UM27 | CLIMB TO REACH [level] BY [position] | Level, Position | WU | No | No | - | Yes | Yes |
| UM28 | DESCEND TO REACH [level] BY [time] | Level, Time | WU | No | No | - | Yes | Yes |
| UM29 | DESCEND TO REACH [level] BY [position] | Level, Position | WU | No | No | - | Yes | Yes |
| UM30 | MAINTAIN BLOCK [level] TO [level] | Level, Level | WU | No | No | - | Yes | No |
| UM31 | CLIMB TO AND MAINTAIN BLOCK [level] TO [level] | Level, Level | WU | No | No | - | Yes | No |
| UM32 | DESCEND TO AND MAINTAIN BLOCK [level] TO [level] | Level, Level | WU | No | No | - | Yes | No |
| UM34 | CRUISE CLIMB TO [level] | Level | WU | No | No | - | Yes | No |
| UM36 | EXPEDITE CLIMB TO [level] | Level | WU | No | No | - | Yes | No |
| UM37 | EXPEDITE DESCENT TO [level] | Level | WU | No | No | - | Yes | No |
| UM38 | IMMEDIATELY CLIMB TO [level] | Level | WU | No | No | - | Yes | No |
| UM39 | IMMEDIATELY DESCEND TO [level] | Level | WU | No | No | - | Yes | No |
| UM46 | CROSS [position] AT [level] | Position, Level | WU | No | No | - | Yes | Yes |
| UM47 | CROSS [position] AT OR ABOVE [level] | Position, Level | WU | No | No | - | Yes | Yes |
| UM48 | CROSS [position] AT OR BELOW [level] | Position, Level | WU | No | No | - | Yes | Yes |
| UM49 | CROSS [position] AT AND MAINTAIN [level] | Position, Level | WU | No | No | - | Yes | No |
| UM50 | CROSS [position] BETWEEN [level] AND [level] | Position, Level, Level | WU | No | No | - | Yes | No |
| UM51 | CROSS [position] AT [time] | Position, Time | WU | No | No | - | Yes | Yes |
| UM52 | CROSS [position] AT OR BEFORE [time] | Position, Time | WU | No | No | - | Yes | Yes |
| UM53 | CROSS [position] AT OR AFTER [time] | Position, Time | WU | No | No | - | Yes | Yes |
| UM54 | CROSS [position] BETWEEN [time] AND [time] | Position, Time, Time | WU | No | No | - | Yes | Yes |
| UM55 | CROSS [position] AT [speed] | Position, Speed | WU | No | No | - | Yes | Yes |
| UM61 | CROSS [position] AT AND MAINTAIN [level] AT [speed] | Position, Level, Speed | WU | No | No | - | Yes | Yes |
| UM64 | OFFSET [distance] [direction] OF ROUTE | Distance, Direction | WU | No | No | - | Yes | Yes |
| UM67 | PROCEED BACK ON ROUTE | - | WU | No | No | - | Yes | No |
| UM74 | PROCEED DIRECT TO [position] | Position | WU | No | No | - | Yes | Yes |
| UM79 | CLEARED TO [position] VIA [route clearance] | Position, RouteClearance | WU | No | No | - | Yes | Yes |
| UM80 | CLEARED [route clearance] | RouteClearance | WU | No | No | - | Yes | Yes |
| UM81 | CLEARED [procedure name] | ProcedureName | WU | No | No | - | Yes | No |
| UM82 | CLEARED TO DEVIATE UP TO [distance] [direction] OF ROUTE | Distance, Direction | WU | No | No | - | Yes | Yes |
| UM92 | HOLD AT [position] AS PUBLISHED MAINTAIN [level] | Position, Level | WU | No | No | - | Yes | Yes |
| UM94 | TURN [direction] HEADING [degrees] | Direction, Degrees | WU | No | No | - | Yes | Yes |
| UM96 | CONTINUE PRESENT HEADING | - | WU | No | No | - | Yes | Yes |
| UM106 | MAINTAIN [speed] | Speed | WU | No | No | - | Yes | Yes |
| UM107 | MAINTAIN PRESENT SPEED | - | WU | No | No | - | Yes | Yes |
| UM108 | MAINTAIN [speed] OR GREATER | Speed | WU | No | No | - | Yes | Yes |
| UM109 | MAINTAIN [speed] OR LESS | Speed | WU | No | No | - | Yes | Yes |
| UM116 | RESUME NORMAL SPEED | - | WU | No | No | - | Yes | Yes |
| UM117 | CONTACT [unit name] [frequency] | UnitName, Frequency | WU | No | No | - | Yes | Yes |
| UM120 | MONITOR [unit name] [frequency] | UnitName, Frequency | WU | No | No | - | Yes | Yes |
| UM123 | SQUAWK [code] | Code | WU | No | No | - | Yes | Yes |
| UM128 | REPORT LEAVING [level] | Level | R | No | No | - | Yes | No |
| UM129 | REPORT MAINTAINING [level] | Level | R | No | No | - | Yes | No |
| UM130 | REPORT PASSING [position] | Position | R | No | No | - | Yes | No |
| UM132 | REPORT POSITION | - | Y | No | No | - | Yes | No |
| UM133 | REPORT PRESENT LEVEL | - | Y | No | No | - | Yes | Yes |
| UM135 | CONFIRM ASSIGNED LEVEL | - | Y | No | No | - | Yes | Yes |
| UM148 | WHEN CAN YOU ACCEPT [level] | Level | Y | No | No | - | Yes | Yes |
| UM149 | CAN YOU ACCEPT [level] AT [position] | Level, Position | AN | No | No | - | Yes | No |
| UM153 | ALTIMETER [altimeter] | Altimeter | R | No | No | - | Yes | No |
| UM158 | ATIS [atis code] | AtisCode | R | No | No | - | Yes | No |
| UM159 | ERROR [error information] | ErrorInfo | N | No | No | - | Yes | Yes |
| UM160 | NEXT DATA AUTHORITY [facility designation] | FacilityDesignation | N | No | No | - | Yes | Yes |
| UM161 | END SERVICE | - | N | No | No | - | Yes | No |
| UM162 | MESSAGE NOT SUPPORTED BY THIS ATS UNIT | - | N | No | No | - | Yes | Yes |
| UM163 | [facility designation] | FacilityDesignation | N | No | No | - | Yes | No |
| UM168 | DISREGARD | - | R | No | No | - | Yes | No |
| UM169 | [free text] | FreeText | R | No | No | - | Yes | Yes |
| UM176 | MAINTAIN OWN SEPARATION AND VMC | - | WU | No | No | - | Yes | No |
| UM179 | SQUAWK IDENT | - | WU | No | No | - | Yes | Yes |
| UM183 | [free text] | FreeText | WU | No | No | - | Yes | Yes |
| UM190 | FLY HEADING [degrees] | Degrees | WU | No | No | - | Yes | Yes |
| UM211 | REQUEST FORWARDED | - | N | No | No | - | Yes | Yes |
| UM215 | TURN [direction] [degrees] DEGREES | Direction, Degrees | WU | No | No | - | Yes | Yes |
| UM222 | NO SPEED RESTRICTION | - | R | No | No | - | Yes | Yes |
| UM227 | LOGICAL ACKNOWLEDGEMENT | - | N | No | No | - | No | Yes |

## Downlink messages (DM)

| ID | Template | Args | Resp | Closing | Standby | Constrained replies | FANS | ATN B1 |
|---|---|---|---|---|---|---|---|---|
| DM0 | WILCO | - | N | Yes | No | - | Yes | Yes |
| DM1 | UNABLE | - | N | Yes | No | - | Yes | Yes |
| DM2 | STANDBY | - | N | No | Yes | - | Yes | Yes |
| DM3 | ROGER | - | N | Yes | No | - | Yes | Yes |
| DM4 | AFFIRM | - | N | Yes | No | - | Yes | Yes |
| DM5 | NEGATIVE | - | N | Yes | No | - | Yes | Yes |
| DM6 | REQUEST [level] | Level | Y | No | No | UM0, UM19, UM20, UM23, UM26, UM27, UM28, UM29, UM46, UM47, UM48 | Yes | Yes |
| DM7 | REQUEST BLOCK [level] TO [level] | Level, Level | Y | No | No | UM0, UM30, UM31, UM32 | Yes | No |
| DM9 | REQUEST CLIMB TO [level] | Level | Y | No | No | UM0, UM19, UM20, UM23, UM26, UM27, UM28, UM29, UM46, UM47, UM48 | Yes | Yes |
| DM10 | REQUEST DESCENT TO [level] | Level | Y | No | No | UM0, UM19, UM20, UM23, UM26, UM27, UM28, UM29, UM46, UM47, UM48 | Yes | Yes |
| DM15 | REQUEST OFFSET [distance] [direction] OF ROUTE | Distance, Direction | Y | No | No | - | Yes | No |
| DM18 | REQUEST [speed] | Speed | Y | No | No | UM0, UM106, UM107, UM108, UM109, UM116, UM222 | Yes | Yes |
| DM20 | REQUEST VOICE CONTACT | - | Y | No | No | - | Yes | No |
| DM22 | REQUEST DIRECT TO [position] | Position | Y | No | No | UM0, UM74, UM96, UM190 | Yes | Yes |
| DM25 | REQUEST CLEARANCE | - | Y | No | No | - | Yes | No |
| DM27 | REQUEST WEATHER DEVIATION UP TO [distance] [direction] OF ROUTE | Distance, Direction | Y | No | No | UM0, UM82, UM64, UM74, UM96, UM190 | Yes | Yes |
| DM28 | LEAVING [level] | Level | N | No | No | - | Yes | No |
| DM29 | CLIMBING TO [level] | Level | N | No | No | - | Yes | No |
| DM30 | DESCENDING TO [level] | Level | N | No | No | - | Yes | No |
| DM31 | PASSING [position] | Position | N | No | No | - | Yes | No |
| DM32 | PRESENT LEVEL [level] | Level | N | No | No | - | Yes | Yes |
| DM33 | PRESENT POSITION [position] | Position | N | No | No | - | Yes | No |
| DM34 | PRESENT SPEED [speed] | Speed | N | No | No | - | Yes | No |
| DM37 | MAINTAINING [level] | Level | N | No | No | - | Yes | No |
| DM38 | ASSIGNED LEVEL [level] | Level | N | No | No | - | Yes | Yes |
| DM41 | BACK ON ROUTE | - | N | No | No | - | Yes | No |
| DM48 | POSITION REPORT [position report] | PositionReport | N | No | No | - | Yes | No |
| DM55 | PAN PAN PAN | - | Y | No | No | - | Yes | Yes |
| DM56 | MAYDAY MAYDAY MAYDAY | - | Y | No | No | - | Yes | Yes |
| DM58 | CANCEL EMERGENCY | - | Y | No | No | - | Yes | Yes |
| DM62 | ERROR [error information] | ErrorInfo | N | No | No | - | Yes | Yes |
| DM63 | NOT CURRENT DATA AUTHORITY | - | N | No | No | - | Yes | Yes |
| DM65 | DUE TO WEATHER | - | N | No | No | - | Yes | Yes |
| DM66 | DUE TO AIRCRAFT PERFORMANCE | - | N | No | No | - | Yes | Yes |
| DM67 | [free text] | FreeText | R | No | No | - | Yes | Yes |
| DM70 | REQUEST HEADING [degrees] | Degrees | Y | No | No | - | Yes | No |
| DM89 | MONITORING [unit name] [frequency] | UnitName, Frequency | N | No | No | - | Yes | Yes |
| DM100 | LOGICAL ACKNOWLEDGEMENT | - | N | No | No | - | No | Yes |

