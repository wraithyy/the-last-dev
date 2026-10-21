# Music mapping proposal

Status: files are in `assets/audio/music/`, credits and licence texts are written, `audio.rs` is not wired yet.
Every track was picked from its catalog description and licence page; none was auditioned. Do a listening pass
before wiring (see "Open questions").

All new tracks are CC BY 4.0 or CC BY 3.0 (credit required, listed in `assets/CREDITS.md` and the site credits).
Two older CC0 tracks stay. Encoding: OGG Vorbis, stereo, 44.1 kHz, `-q:a 0` (about 64-88 kbps). Total new music
about 25 MB, 19 files in the folder.

## Zone to track

| Zone | File | Mood | Source |
|---|---|---|---|
| campus | `campus.ogg` | calm, airy, future-ambient | Floating Cities, Kevin MacLeod, CC BY 4.0 |
| parking_p3 | `parking_p3.ogg` | slow, empty, late-night highway | Limit 70, Kevin MacLeod, CC BY 4.0 |
| tower/1 Open Space | `open_space.ogg` | clean, bland corporate lo-fi | Wallpaper, Kevin MacLeod, CC BY 4.0 |
| tower/2 Meeting Rooms | `office_muzak.ogg` (existing) | elevator muzak | Elevator Music, Alex McCulloch, CC0 |
| tower/3 Sales & Marketing | `sales_marketing.ogg` | upbeat funk, adult | Funkorama, Kevin MacLeod, CC BY 4.0 |
| tower/4 PMO | `pmo.ogg` | slow, suspicious, bureaucratic scheming | Covert Affair, Kevin MacLeod, CC BY 4.0 |
| tower/5 C-Level Penthouse | `c_level_penthouse.ogg` | cold, dark, driving, calm | Tyrant, Kevin MacLeod, CC BY 4.0 |
| datacenter/1 | `datacenter_hall_a.ogg` | tense electronic loop | Mesmerizing Galaxy (Loop), Kevin MacLeod, CC BY 4.0 |
| datacenter/2 | `datacenter_hall_b.ogg` | thick, aggressive electronic | Ouroboros, Kevin MacLeod, CC BY 4.0 |
| lab/1 Showroom | `lab_showroom.ogg` | synthwave downtempo | Moonlit Skyline, Nihilore, CC BY 4.0 |
| lab/2 Rack Room | `lab_rack_room.ogg` | dark, moody, suspenseful | Crypto, Kevin MacLeod, CC BY 4.0 |
| home/1 | `home.ogg` | soft evening calm | Evening, Kevin MacLeod, CC BY 4.0 |
| offsite/1, offsite/2 | `offsite_lobby.ogg` | smooth lobby jazz | Lobby Time, Kevin MacLeod, CC BY 4.0 |
| offsite/3 Terrace | `campus.ogg` (reuse) | calm outdoors | see campus |
| offsite/4 Ballroom (boss) | `raid_boss_offsite.ogg` | static dark tension, violent drum hits | Perpetual Tension, Zander Noriega, CC BY 3.0 |
| vibecon/1, 2, 3 | `vibecon_expo.ogg` | driving, grooving, conference energy | Brain Dance, Kevin MacLeod, CC BY 4.0 |
| vibecon/4 Afterparty | `vibecon_afterparty.ogg` | 2 am neon club, mysterious | Ethernight Club, Kevin MacLeod, CC BY 4.0 |
| vibecon/5 Main Stage (boss) | `raid_boss_vibecon.ogg` | cinematic electronic, modern drums | Wasteland Overdrive (Looped, no choir), Matthew Pablo, CC BY 3.0 |
| client/1 | `open_space.ogg` (reuse) | neutral corporate | see tower/1 |
| teambuilding/1 | `sales_marketing.ogg` (reuse) | forced fun, but adult | see tower/3 |
| review/1 | `pmo.ogg` (reuse) | tense | see tower/4 |
| reorg/1 | `lab_rack_room.ogg` (reuse) | anxious | see lab/2 |
| migration/1 | `datacenter_hall_a.ogg` (reuse) | back-office tension | see datacenter/1 |
| hackathon/1 | `vibecon_expo.ogg` (reuse) | driving | see vibecon/1 |
| openspace/1 | `open_space.ogg` (reuse) | bland | see tower/1 |
| night variant (any outdoor zone) | `night.ogg` | eerie, inky texture | Aftermath, Kevin MacLeod, CC BY 4.0 |
| generic boss fight (non-raid) | `boss_battle.ogg` (existing) | chiptune-ish epic | Epic Boss Battle, Juhani Junkala, CC0 |

Gaps: no separate combat (non-boss) track, no distinct tower or datacenter boss track. `boss_battle.ogg` still
covers those and is the most "game-y" track left; replace it if it sounds too cartoonish next to the rest.

## Links and licence evidence

- Kevin MacLeod: tracks listed in `assets/audio/licenses/incompetech-kevin-macleod.txt`; licence CC BY 4.0
  (https://creativecommons.org/licenses/by/4.0/) per https://incompetech.com/music/royalty-free/licenses/ and the
  catalog https://incompetech.com/music/royalty-free/pieces.json (the licensing block names CC BY 4.0 for every piece).
  Source files: `https://incompetech.com/music/royalty-free/mp3-royaltyfree/<filename>`.
- Nihilore, Moonlit Skyline: https://www.nihilore.com/downtempo, licence page https://www.nihilore.com/license
  (CC BY 4.0, "Absolutely No AI was or ever will be used", track names must not be changed).
- Wasteland Overdrive: https://opengameart.org/content/wasteland-overdrive, CC-BY 3.0, composer Matthew Pablo.
- Perpetual Tension: https://opengameart.org/content/perpetual-tension, CC-BY 3.0, composer Zander Noriega.

## Rejected

- Pixabay music: not a Creative Commons licence.
- Digital Evening (OGA, looneybits, CC-BY 3.0): fits the Rack Room mood, but it is unclear whether the uploader is
  the composer, so the credit could be wrong. Replaced by Crypto.
- Robotic City V2, Dark City (OGA, CC0): hobbyist quality, short loops, uncertain tone.
- Lofi tracks on OGA tagged "8 bit" (Since 2 A.M., Peaceful 1am in May, ...): chiptune, too cartoonish for the brief.
- Chill Lofi Inspired, Lofi Hip Hop (OGA, CC0): keyboard-demo sound, not polished.
- Local Forecast - Elevator, Quirky Dog, Cloud Dancer, Digital Lemonade (Kevin MacLeod): bouncy or humorous feel.
- Nihilore tracks beyond Moonlit Skyline: pool is large (downtempo, industrial, lounge), but many have vocals or
  profane titles ("Fuck Voluntary Compliance") and titles may not be changed. Candidates for a later pass after listening:
  Satellites and Waterfalls, Sleepwalking, Fatigue, Toska, Saudade.
- Boss tracks on OGA tagged chiptune/8-bit/orchestral-fantasy (Boss Battle Music, Colossal Boss Battle Theme, ...): tone.
- Not checked (licence usually NC/ND): Lobo Loco, Podington Bear, Ketsa, Blue Dot Sessions. Scott Buckley (CC BY 4.0,
  cinematic) and Broke For Free were not evaluated; both are good sources for a second round.

## Open questions

- Loops: only Mesmerizing Galaxy (Loop) and Wasteland Overdrive (Looped) are authored as seamless loops. The others
  are full tracks of 1.5-5.7 min with a natural ending, so `PlaybackSettings::LOOP` will cut at the end. Wire them with
  the existing crossfade (fade out near the end, restart) or trim and crossfade the tail in ffmpeg.
- Nothing was listened to. Check especially Funkorama (funk may sound too cheerful), Ouroboros, Brain Dance and the
  length of Tyrant (5.7 min, 2.7 MB).
- Attribution must stay in the credits for as long as the files ship. CC BY 3.0 and 4.0 both require a licence link.
- Per-zone selection needs a new `Track` mapping in `audio.rs` (currently only Explore, Boss, Ambient).
