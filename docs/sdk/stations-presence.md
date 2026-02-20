# Stations and presence

## Purpose

Define how your client should publish and consume station availability information.

## Presence lifecycle

Recommended sequence:

1. Connect to NATS
2. Activate inbox subscription
3. Publish station `online`
4. Run normal CPDLC operations
5. Publish `offline` on graceful shutdown

## Presence payload essentials

- stable station identifier,
- status (`online`/`offline`),
- linked operational identity (callsign + ACARS address).

## Integration rules

- Do not expose operational actions before inbox is active.
- Re-publish `online` after reconnect.
- Keep station identifier stable across sessions when possible.

## Example behavior

- If reconnect occurs, restore inbox subscription first, then publish `online` again.
- If a destination is offline, disable contact/start actions or show a clear warning.

## Common mistakes

- marking online before inbox is subscribed,
- changing station identity at each reconnect,
- ignoring stale online state after network interruption.

## Related pages

- [Addressing and routing](addressing-routing.md)
- [Integration checklist](integration-checklist.md)
