# Future AIRBAND: passive location hypotheses

**Design proposal only. This feature is not implemented.** The shipped AIRBAND
app's Nearby list uses a user-selected airport as its center. It does not infer
where the radio is, decode VOR bearings, or provide a navigation position.

This work should remain on a separate research branch from the hardware-tested
AIRBAND release. The existing directory, receiver and manual Nearby functions
are useful without automatic location and should not be destabilized by an
unvalidated inference experiment.

## Project boundary

The first useful result is not an on-radio “you are here” screen. It is a
reproducible host-side survey that shows which observations help, which are
ambiguous, and when no conclusion is justified. The project must not:

- claim GPS-, triangulation- or navigation-equivalent accuracy;
- turn RSSI directly into distance;
- assume the nearest database assignment owns a reused frequency;
- treat silence as proof that a facility is distant or closed;
- infer a VOR radial without phase measurements; or
- transmit probes on aviation frequencies.

A future UI should say “candidate area” or “possible stations,” not “location,”
until field validation supports a stronger statement.

## Intended behavior

A future app could listen to a set of frequencies and suggest geographic areas
consistent with the observations. Its output would be a short list of candidate
areas, the station associations supporting each, conflicting observations, and
an explicit insufficient-evidence state. It should avoid precise coordinates
or percentage confidence until measured field performance supports them.

Useful observations include persistent ATIS/AWOS carriers and positively
identified VOR stations. Shared CTAF/UNICOM frequencies provide little geographic
specificity. Hearing a tower frequency alone does not establish that the tower
is the transmitter: an airborne aircraft may be the station being received.

## Why frequency and RSSI are insufficient for a position fix

- Frequencies are reused by geographically separated facilities.
- Transmitter power, antenna pattern, terrain, altitude, buildings, multipath,
  receiver gain and the user's antenna affect RSSI. It is not a distance sensor.
- Silence is weak evidence: traffic is intermittent, frequencies are sectorized,
  and facilities may be closed, out of service, or outside radio line of sight.
- FAA airport coordinates describe an airport reference point. They must not
  silently be treated as the coordinates of a remote communications transmitter.
- One receiver cannot derive a bearing or arrival-time difference merely from
  sequential frequency scans. This proposal is not triangulation or VOR radial
  decoding.

The FAA describes VOR line-of-sight/altitude limitations and identification by
[Morse or automatic voice identification](https://www.faa.gov/air_traffic/publications/atpubs/aip_html/chap1_section_1.html).
Database-expected identity and positively received identity must be distinct
fields in any future UI.

## Hardware and API prerequisites

The existing app ABI supplies frequency tuning, AM selection, RSSI, squelch
state, timing and external-flash storage. That supports a carrier survey.
It **does not expose received PCM audio**, nor bearing, time-of-flight or GPS
measurements. The microphone ADC callback is not a received-audio stream.
The USB programming connection carries serial protocol, not PCM audio.

Automatic VOR Morse identification would first require an experimentally
verified receive-audio acquisition path and sufficient filtering/sample rate.
Prototype decoding on a computer from an appropriate receive-audio connection
before proposing an on-radio ADC/DMA path or hardware modification. Do not assume
that the RF chip's DTMF/FSK decoder can decode arbitrary Morse identification.
A successful Morse decoder would identify a station; it would not supply its
bearing or range.

The current CPS programming mode shuts RF down. A measurement logger should
record a survey locally, then export it in a separate programming session, or
use a newly designed telemetry interface. It cannot collect normal RF survey
data while the existing CPS programming session is running.

## Data model

A future database needs more than the compact identifier directory:

- Frequency-to-transmitter associations, including every plausible reused
  assignment, actual site coordinates when available, and coordinate quality.
- Facility/service type, operational status, effective cycle and available
  service-volume information.
- Expected VOR identifiers and the origin of each identification observation.
- Optional user-selected starting region or airport, labeled as a prior rather
  than a measured radio position.

Surveys should store timestamp, tuned frequency, modulation/bandwidth, antenna
profile if known, settled RSSI samples, noise-floor estimate, carrier duration,
squelch state, and any independently decoded identification. Reserve a separate
flash partition or use host storage; never borrow the database, favorites,
calibration, app slots, or TEXT log without an explicit layout migration.

## Proposed implementation stages

1. **Host-side survey explorer.** Export measured carrier observations to a Mac.
   Overlay possible station associations and expose ambiguity for human review.
2. **Candidate-area ranking.** Generate geographical cells/regions from the
   database. Rank combinations of plausible ground stations rather than choosing
   the nearest database entry for every frequency. Use signal presence as soft
   evidence; weight continuous, specific stations more strongly. Do not eliminate
   regions solely because a channel was silent.
3. **Identity-assisted ranking.** Add independently validated Morse/voice station
   identification if receive-audio access proves practical. Keep unconfirmed
   carrier hits separate from identified stations.
4. **On-radio prototype.** Move a bounded search and the compact candidate list
   into a segmented app only after the host algorithm has useful measured results.
   Offer manual region selection, survey cancellation, repeat scans, and an
   insufficient-evidence result.

## Recommended development workflow

1. Create a separate branch such as `airband-passive-location-research` from a
   tagged, working AIRBAND release. Do not put experimental flash-layout changes
   in the stable AIRBAND PR.
2. Define a versioned, append-only survey format before implementing ranking.
   JSON Lines or CSV is sufficient initially. Include firmware/app/database
   hashes so old measurements remain interpretable after software changes.
3. Add a bounded on-radio carrier logger only if current RSSI/squelch APIs can
   collect stable measurements without disrupting receiver operation. Export the
   log after the survey, not during CPS mode.
4. Build visualization and ranking on the host first. Preserve every plausible
   frequency assignment and display contradictory evidence.
5. Publish anonymized fixtures and synthetic tests; keep raw location-tagged
   field logs private unless the collector explicitly chooses to share them.
6. Establish acceptance thresholds on held-out locations before moving ranking
   onto the radio. A result that frequently says “insufficient evidence” is
   preferable to false precision.
7. Open this as a separate proposal/PR only after the stable AIRBAND work is
   accepted or clearly maintained in a fork.

A minimal survey record should contain a schema version, timestamp, known test
location kept separately or redacted for public data, antenna/setup label,
database hash, tuned frequency, modulation, dwell duration, RSSI samples,
noise-floor estimate, squelch transitions, carrier duration and independently
verified station identity if available. Do not store an inferred station ID in
the same field as a decoded or human-confirmed ID.

## Validation and acceptance criteria

Collect ground-truth surveys at known locations with several antennas, indoors
and outdoors, urban and rural terrain, near and far from facilities, and at
different times. Include repeated frequencies, a moving airborne transmitter,
strong unrelated signals, missing/outdated assignments, and an empty band.
Hold out entire locations from algorithm tuning.

Measure candidate-area inclusion, ambiguity, failure rate, scan duration and
false certainty. A common CTAF carrier by itself must never generate a confident
location. Confidence displays require calibration against held-out data.

No accuracy claim is made here. This would be a listening/site-discovery aid;
flight navigation requires an appropriate independent navigation source.

## Separate research: VOR radial decoding

A radial decoder is distinct from passive location ranking or station-ID
recognition. VOR navigation compares the phases of two 30 Hz components, one
recovered through a 9,960 Hz subcarrier; see the
[FAA signal standard](https://www.faa.gov/documentLibrary/media/Advisory_Circular/AC00-31a.pdf).
The current firmware exposes neither a navigation decoder nor received-baseband
samples to apps. A future experiment must first demonstrate that the accessible
signal path preserves those components and their relative phase. Speaker audio
or a programming-cable serial stream must not be assumed suitable.

With a proven acquisition path, prototype subcarrier demodulation, reference/
variable phase extraction, station identification, signal-validity detection and
CVOR/DVOR handling on a computer. Validate against generated signals and known
radials before considering the radio's memory/CPU limits. RSSI, Morse ID or the
manually selected nearby airport are not radial measurements. GPS coordinates
could support a separately labeled geometric calculation, with station alignment
handled correctly, but this radio has no implemented GPS position source.
