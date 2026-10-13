# Asset credits

## 3D models

Both packs are CC0 (https://creativecommons.org/publicdomain/zero/1.0/); attribution is not required but given gladly.

### Characters (Quaternius)

Ultimate Animated Character Pack (Nov 2019) by Quaternius (https://quaternius.com), CC0. The original License.txt is
assets/models/LICENSE-quaternius.txt. The glTF files were repacked into single .glb files
(scripts/gltf_to_glb.py), and the "Skin" and "Face" material colours were changed to skin tones and dark eyes. Suit_Male_Bald.glb (the CEO) is derived from Suit_Male by scripts/ceo_hair.py: short hair, receding hairline, grey-to-black vertex colours.

Nine base models cover the whole cast; per-unit shirt, hair and skin colours are applied at runtime
(`crates/game_client/src/catalog.rs`). Skin tones differ per base model for variety.

| File | Character | Used for |
|---|---|---|
| assets/models/characters/Suit_Male.glb | Suit_Male | CEO, Customer, CISO, Middle Manager, Project Manager |
| assets/models/characters/Suit_Female.glb | Suit_Female | VP of Synergy, Head of Marketing, HR Business Partner, Sales Rep, Account Manager |
| assets/models/characters/Casual3_Male.glb | Casual3_Male | Product Owner |
| assets/models/characters/Engineer.glb | Casual2_Male body with the Casual3_Male hair, recoloured (black tee, jeans) and hair split into Hair_1..Hair_5 by scripts/split_hair.py | Senior Engineer (player) |
| assets/models/characters/Casual_Female.glb | Casual_Female | AI-Native Intern, Agile Coach, LinkedIn Influencer |
| assets/models/characters/Casual2_Male.glb | Casual2_Male | Vibe Coder, Vibe-Coding Junior, Scrum Master, Crypto Bro, Growth Hacker, IT Support |
| assets/models/characters/Casual_Male.glb | Casual_Male | MeatProxy, Copilot Drone, Chatbot Minion, Unreviewed PR |
| assets/models/characters/Doctor_Male_Young.glb | Doctor_Male_Young | The Analyst |
| assets/models/characters/Doctor_Female_Old.glb | Doctor_Female_Old | AI Ethicist |
| assets/models/characters/Worker_Male.glb | Worker_Male | Consultant, Security Analyst |

### Furniture (Kenney)

The original License.txt is in assets/models/LICENSE-kenney.txt.

| Folder | Pack | Source |
|---|---|---|
| assets/models/furniture/ | Kenney Furniture Kit 2.0 | https://kenney.nl/assets/furniture-kit |

Author: Kenney (https://www.kenney.nl).

Used pieces, unmodified: bookcaseClosed, bookcaseClosedWide, bookcaseOpen, books, cardboardBoxClosed, chairDesk,
chairModernCushion, coatRackStanding, computerKeyboard, computerMouse, computerScreen, desk, cabinetTelevision,
benchCushion, kitchenCoffeeMachine, kitchenFridge, lampRoundFloor, lampRoundTable, lampSquareFloor, laptop,
loungeChair, loungeChairRelax, loungeDesignSofa, loungeSofa, loungeSofaLong, plantSmall1-3, pottedPlant,
rugRectangle, rugRound, sideTable, stoolBar, table, tableCoffee, tableCoffeeGlass, tableGlass, tableRound,
televisionModern, trashcan. Everything else (walls, glass, elevators, foosball, shrines, signs) is built from primitives.

## Spell icons (Painterly Spell Icons)

"Painterly Spell Icons" parts 1-3 by J. W. Bjerk (eleazzaar) -- www.jwbjerk.com/art --
found at http://opengameart.org. Used under CC BY 3.0
(https://creativecommons.org/licenses/by/3.0/); downscaled from 256x256 to 128x128.

| File | Source | Link |
|---|---|---|
| assets/icons/code_review.png | part 1, evil-eye-eerie-1 | https://opengameart.org/content/painterly-spell-icons-part-1 |
| assets/icons/refactor.png | part 3, runes-blue-1 | https://opengameart.org/content/painterly-spell-icons-part-3 |
| assets/icons/guardrails.png | part 1, protect-blue-1 | https://opengameart.org/content/painterly-spell-icons-part-1 |
| assets/icons/reject_pr.png | part 2, lighting-fire-1 | https://opengameart.org/content/painterly-spell-icons-part-2 |
| assets/icons/nitpick.png | part 2, ice-sky-1 | https://opengameart.org/content/painterly-spell-icons-part-2 |
| assets/icons/code_smell.png | part 3, horror-acid-1 | https://opengameart.org/content/painterly-spell-icons-part-3 |
| assets/icons/hallucination_blast.png | part 3, explosion-magenta-1 | https://opengameart.org/content/painterly-spell-icons-part-3 |

## UI icons (game-icons.net)

Glyphs are from https://github.com/game-icons/icons, licensed CC BY 3.0
(https://creativecommons.org/licenses/by/3.0/). Each glyph was rasterized to 64x64
and composited onto a coloured gradient with a bevel frame (original styling).

| File | Source glyph | Author folder | Licence | Link |
|---|---|---|---|---|
| assets/icons/prompt_fragment.png | tied-scroll | lorc | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/lorc/tied-scroll.svg |
| assets/icons/melee.png | crossed-swords | lorc | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/lorc/crossed-swords.svg |
| assets/icons/unread_ai_strategy_deck.png | scroll-unfurled | lorc | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/lorc/scroll-unfurled.svg |
| assets/icons/expired_gpu_credits.png | processor | lorc | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/lorc/processor.svg |
| assets/icons/mandatory_prompt_training_certificate.png | diploma | delapouite | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/delapouite/diploma.svg |

Authors: Lorc (https://lorcblog.blogspot.com), Delapouite (https://delapouite.com).

## Procedural textures

The carpet, marble, ceiling tiles, posters, signs, Gantt charts, skyline and screens are generated in code
(`crates/game_client/src/floors/textures.rs`), text included, with a built-in 5x7 bitmap font. No external art or font is used for them.

## Fonts

| File | Font | Licence | Source |
|---|---|---|---|
| assets/fonts/JetBrainsMono-Bold.ttf | JetBrains Mono Bold v2.304, (c) 2020 The JetBrains Mono Project Authors | SIL Open Font License 1.1 (assets/fonts/OFL-JetBrainsMono.txt) | https://github.com/JetBrains/JetBrainsMono |

Icons whose key has no file in `icons/` are generated at runtime as letter badges (no external art).
