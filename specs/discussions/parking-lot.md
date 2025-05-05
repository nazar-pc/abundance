# Parking Lot

> So what exactly is the parking lot? As part of the sync and async discussions that we are having,
> we sometimes come up with "things that we'll need to figure out and describe in detail as we make
> progress with the protocol and that need to be _parked_ for now". The parking lot includes the
> list of things that have come up as important, are under-defined, we are not in a place to design
> and discuss in detail now, but we'll need to in the future. The goal of this list is to prevent
> them from being lost in my meeting notes.

- Use the data availability layer to probabilistically inspect random chunks of pieces in a child
  shard segment so we can validate that the segments submitted to the global history of the system
  are available and they have been generated successfully and has not been forged by a malicious
  node in a compromise shard (or, otherwise, to flag that there is an issue with the segment).
- The data availability layer relies on nodes periodically sampling segments from the global
  history, which requires random nodes to request segments from farmers and notify when one is not
  valid. Challenging a segment as unavailable or incorrect should have some price or be limited in
  some way to prevent DDoS attacks.
