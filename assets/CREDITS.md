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
televisionModern, trashcan. Everything else (walls, glass, elevators, foosball, shrines, signs, the whole campus,
data center and innovation lab, and the primitive-built units in `crates/game_client/src/rigs.rs`) is built from
primitives and procedural textures; no new assets were added for those zones.

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
| assets/icons/ab_loser.png | part 3, wild-eerie-1 | https://opengameart.org/content/painterly-spell-icons-part-3 |
| assets/icons/ab_test.png | part 3, wild-orange-1 | https://opengameart.org/content/painterly-spell-icons-part-3 |
| assets/icons/ab_winner.png | part 3, wild-jade-1 | https://opengameart.org/content/painterly-spell-icons-part-3 |
| assets/icons/agi_pivot.png | part 3, explosion-royal-1 | https://opengameart.org/content/painterly-spell-icons-part-3 |
| assets/icons/analysis.png | part 1, evil-eye-red-1 | https://opengameart.org/content/painterly-spell-icons-part-1 |
| assets/icons/big_diff.png | part 1, fireball-red-1 | https://opengameart.org/content/painterly-spell-icons-part-1 |
| assets/icons/billable_hours.png | part 1, heal-royal-1 | https://opengameart.org/content/painterly-spell-icons-part-1 |
| assets/icons/block_all.png | part 1, protect-red-1 | https://opengameart.org/content/painterly-spell-icons-part-1 |
| assets/icons/caffeine.png | part 2, haste-fire-1 | https://opengameart.org/content/painterly-spell-icons-part-2 |
| assets/icons/chatbot_rollout.png | part 2, beam-sky-1 | https://opengameart.org/content/painterly-spell-icons-part-2 |
| assets/icons/culture_fit.png | part 3, horror-eerie-1 | https://opengameart.org/content/painterly-spell-icons-part-3 |
| assets/icons/deadline.png | part 1, fireball-eerie-1 | https://opengameart.org/content/painterly-spell-icons-part-1 |
| assets/icons/ethics_review.png | part 1, enchant-blue-1 | https://opengameart.org/content/painterly-spell-icons-part-1 |
| assets/icons/family.png | part 3, leaf-jade-1 | https://opengameart.org/content/painterly-spell-icons-part-3 |
| assets/icons/forwarded_reply.png | part 2, beam-blue-1 | https://opengameart.org/content/painterly-spell-icons-part-2 |
| assets/icons/generate_prs.png | part 3, runes-orange-1 | https://opengameart.org/content/painterly-spell-icons-part-3 |
| assets/icons/git_blame.png | part 2, beam-red-1 | https://opengameart.org/content/painterly-spell-icons-part-2 |
| assets/icons/hallucinated_hotfix.png | part 3, explosion-orange-1 | https://opengameart.org/content/painterly-spell-icons-part-3 |
| assets/icons/influencer_collab.png | part 1, enchant-magenta-1 | https://opengameart.org/content/painterly-spell-icons-part-1 |
| assets/icons/locked_out.png | part 1, protect-eerie-1 | https://opengameart.org/content/painterly-spell-icons-part-1 |
| assets/icons/overpromise.png | part 1, enchant-orange-1 | https://opengameart.org/content/painterly-spell-icons-part-1 |
| assets/icons/password_rotation.png | part 3, runes-royal-1 | https://opengameart.org/content/painterly-spell-icons-part-3 |
| assets/icons/phished.png | part 3, horror-red-1 | https://opengameart.org/content/painterly-spell-icons-part-3 |
| assets/icons/phishing.png | part 2, lightning-blue-1 | https://opengameart.org/content/painterly-spell-icons-part-2 |
| assets/icons/postmortem.png | part 3, explosion-red-1 | https://opengameart.org/content/painterly-spell-icons-part-3 |
| assets/icons/press_release.png | part 2, lightning-orange-1 | https://opengameart.org/content/painterly-spell-icons-part-2 |
| assets/icons/quick_sync.png | part 2, ice-blue-1 | https://opengameart.org/content/painterly-spell-icons-part-2 |
| assets/icons/rebrand.png | part 3, wind-magenta-1 | https://opengameart.org/content/painterly-spell-icons-part-3 |
| assets/icons/rebranded.png | part 1, protect-magenta-1 | https://opengameart.org/content/painterly-spell-icons-part-1 |
| assets/icons/relationship_building.png | part 1, heal-sky-1 | https://opengameart.org/content/painterly-spell-icons-part-1 |
| assets/icons/reorg.png | part 3, wind-red-1 | https://opengameart.org/content/painterly-spell-icons-part-3 |
| assets/icons/reprioritize.png | part 3, wind-blue-1 | https://opengameart.org/content/painterly-spell-icons-part-3 |
| assets/icons/requirements_change.png | part 3, runes-magenta-1 | https://opengameart.org/content/painterly-spell-icons-part-3 |
| assets/icons/retrospective.png | part 1, enchant-jade-1 | https://opengameart.org/content/painterly-spell-icons-part-1 |
| assets/icons/review_fatigue.png | part 2, vines-eerie-1 | https://opengameart.org/content/painterly-spell-icons-part-2 |
| assets/icons/review_request.png | part 2, vines-plain-1 | https://opengameart.org/content/painterly-spell-icons-part-2 |
| assets/icons/rollback.png | part 1, heal-jade-1 | https://opengameart.org/content/painterly-spell-icons-part-1 |
| assets/icons/rubber_duck.png | part 2, rock-orange-1 | https://opengameart.org/content/painterly-spell-icons-part-2 |
| assets/icons/rug_pull.png | part 2, rock-sky-1 | https://opengameart.org/content/painterly-spell-icons-part-2 |
| assets/icons/scope_creep.png | part 2, vines-acid-1 | https://opengameart.org/content/painterly-spell-icons-part-2 |
| assets/icons/soc.png | part 2, lighting-royal-1 | https://opengameart.org/content/painterly-spell-icons-part-2 |
| assets/icons/stakeholders.png | part 2, haste-royal-1 | https://opengameart.org/content/painterly-spell-icons-part-2 |
| assets/icons/standup.png | part 2, ice-jade-1 | https://opengameart.org/content/painterly-spell-icons-part-2 |
| assets/icons/synergy_storm.png | part 3, wind-sky-1 | https://opengameart.org/content/painterly-spell-icons-part-3 |
| assets/icons/thought_leadership.png | part 2, beam-magenta-1 | https://opengameart.org/content/painterly-spell-icons-part-2 |
| assets/icons/unread_spec.png | part 1, enchant-eerie-1 | https://opengameart.org/content/painterly-spell-icons-part-1 |
| assets/icons/viral_loop.png | part 3, leaf-acid-1 | https://opengameart.org/content/painterly-spell-icons-part-3 |
| assets/icons/wellness_survey.png | part 1, protect-orange-1 | https://opengameart.org/content/painterly-spell-icons-part-1 |
| assets/icons/write_by_hand.png | part 2, lightning-magenta-1 | https://opengameart.org/content/painterly-spell-icons-part-2 |
| assets/icons/yesterday.png | part 1, fireball-sky-1 | https://opengameart.org/content/painterly-spell-icons-part-1 |
| assets/icons/zero_trust.png | part 1, protect-sky-1 | https://opengameart.org/content/painterly-spell-icons-part-1 |
| assets/icons/recalculating_route.png | part 3, runes-blue-2 | https://opengameart.org/content/painterly-spell-icons-part-3 |
| assets/icons/hit_and_run.png | part 2, rock-plain-1 | https://opengameart.org/content/painterly-spell-icons-part-2 |
| assets/icons/free_swag.png | part 3, leaf-royal-1 | https://opengameart.org/content/painterly-spell-icons-part-3 |
| assets/icons/brand_awareness.png | part 1, enchant-royal-1 | https://opengameart.org/content/painterly-spell-icons-part-1 |
| assets/icons/batch_blessing.png | part 2, haste-sky-1 | https://opengameart.org/content/painterly-spell-icons-part-2 |
| assets/icons/exhaust_gust.png | part 3, wind-red-2 | https://opengameart.org/content/painterly-spell-icons-part-3 |
| assets/icons/context_window.png | part 2, beam-eerie-1 | https://opengameart.org/content/painterly-spell-icons-part-2 |
| assets/icons/agentic_copy.png | part 2, vines-jade-1 | https://opengameart.org/content/painterly-spell-icons-part-2 |
| assets/icons/job_pending.png | part 2, ice-sky-2 | https://opengameart.org/content/painterly-spell-icons-part-2 |
| assets/icons/preemption.png | part 2, lighting-sky-1 | https://opengameart.org/content/painterly-spell-icons-part-2 |
| assets/icons/fair_share.png | part 3, explosion-sky-1 | https://opengameart.org/content/painterly-spell-icons-part-3 |
| assets/icons/overheating.png | part 1, fireball-red-2 | https://opengameart.org/content/painterly-spell-icons-part-1 |
| assets/icons/rack_failover.png | part 2, beam-orange-1 | https://opengameart.org/content/painterly-spell-icons-part-2 |
| assets/icons/thermal_event.png | part 3, explosion-orange-3 | https://opengameart.org/content/painterly-spell-icons-part-3 |
| assets/icons/hot_aisle.png | part 2, lightning-orange-2 | https://opengameart.org/content/painterly-spell-icons-part-2 |
| assets/icons/live_demo.png | part 1, fireball-acid-1 | https://opengameart.org/content/painterly-spell-icons-part-1 |
| assets/icons/series_b.png | part 3, leaf-orange-1 | https://opengameart.org/content/painterly-spell-icons-part-3 |
| assets/icons/demo_works.png | part 1, protect-jade-1 | https://opengameart.org/content/painterly-spell-icons-part-1 |
| assets/icons/demo_crashed.png | part 3, horror-red-3 | https://opengameart.org/content/painterly-spell-icons-part-3 |
| assets/icons/prompt_injected.png | part 2, lightning-jade-1 | https://opengameart.org/content/painterly-spell-icons-part-2 |
| assets/icons/jailbreak.png | part 2, lighting-eerie-1 | https://opengameart.org/content/painterly-spell-icons-part-2 |
| assets/icons/confident_answer.png | part 1, enchant-acid-1 | https://opengameart.org/content/painterly-spell-icons-part-1 |
| assets/icons/hallucinate_copies.png | part 3, wild-acid-1 | https://opengameart.org/content/painterly-spell-icons-part-3 |
| assets/icons/emergent_capability.png | part 1, evil-eye-eerie-3 | https://opengameart.org/content/painterly-spell-icons-part-1 |
| assets/icons/alignment_training.png | part 1, protect-acid-1 | https://opengameart.org/content/painterly-spell-icons-part-1 |
| assets/icons/self_improvement.png | part 1, enchant-red-1 | https://opengameart.org/content/painterly-spell-icons-part-1 |

## UI icons (game-icons.net)

Glyphs are from https://github.com/game-icons/icons, licensed CC BY 3.0
(https://creativecommons.org/licenses/by/3.0/). Each glyph was rasterized to 64x64
and composited onto a coloured gradient with a bevel frame (original styling). Trophy icons are coloured by the
bonus they grant (red HP, blue mana, violet spell power, gold crit, teal haste, green mana regen).

| File | Source glyph | Author folder | Licence | Link |
|---|---|---|---|---|
| assets/icons/prompt_fragment.png | tied-scroll | lorc | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/lorc/tied-scroll.svg |
| assets/icons/melee.png | crossed-swords | lorc | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/lorc/crossed-swords.svg |
| assets/icons/unread_ai_strategy_deck.png | scroll-unfurled | lorc | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/lorc/scroll-unfurled.svg |
| assets/icons/expired_gpu_credits.png | processor | lorc | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/lorc/processor.svg |
| assets/icons/mandatory_prompt_training_certificate.png | diploma | delapouite | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/delapouite/diploma.svg |
| assets/icons/chat_bubble.png | chat-bubble | delapouite | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/delapouite/chat-bubble.svg |
| assets/icons/wrong_thread.png | conversation | lorc | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/lorc/conversation.svg |
| assets/icons/regen.png | rolling-energy | lorc | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/lorc/rolling-energy.svg |
| assets/icons/pull_request.png | cog | lorc | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/lorc/cog.svg |
| assets/icons/zip_file.png | archive-register | delapouite | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/delapouite/archive-register.svg |
| assets/icons/accepted.png | check-mark | delapouite | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/delapouite/check-mark.svg |
| assets/icons/keyboard.png | laptop | delapouite | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/delapouite/laptop.svg |
| assets/icons/tests_off.png | pencil | delapouite | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/delapouite/pencil.svg |
| assets/icons/chart.png | chart | delapouite | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/delapouite/chart.svg |
| assets/icons/sticky.png | notebook | delapouite | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/delapouite/notebook.svg |
| assets/icons/manifesto.png | papers | lorc | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/lorc/papers.svg |
| assets/icons/calendar.png | calendar | delapouite | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/delapouite/calendar.svg |
| assets/icons/badge.png | id-card | delapouite | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/delapouite/id-card.svg |
| assets/icons/pizza.png | pizza-slice | delapouite | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/delapouite/pizza-slice.svg |
| assets/icons/pto.png | ticket | delapouite | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/delapouite/ticket.svg |
| assets/icons/wellness.png | chained-heart | lorc | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/lorc/chained-heart.svg |
| assets/icons/brain.png | brain | lorc | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/lorc/brain.svg |
| assets/icons/chart_up.png | progression | delapouite | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/delapouite/progression.svg |
| assets/icons/spiral.png | spiral-bloom | lorc | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/lorc/spiral-bloom.svg |
| assets/icons/contract.png | trade | lorc | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/lorc/trade.svg |
| assets/icons/box.png | cardboard-box | delapouite | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/delapouite/cardboard-box.svg |
| assets/icons/megaphone.png | megaphone | delapouite | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/delapouite/megaphone.svg |
| assets/icons/hook.png | fishing-hook | lorc | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/lorc/fishing-hook.svg |
| assets/icons/brand.png | star-swirl | lorc | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/lorc/star-swirl.svg |
| assets/icons/webinar.png | radar-dish | lorc | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/lorc/radar-dish.svg |
| assets/icons/logo.png | compass | lorc | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/lorc/compass.svg |
| assets/icons/mailbox.png | mailbox | delapouite | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/delapouite/mailbox.svg |
| assets/icons/thumb_down.png | thumb-down | delapouite | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/delapouite/thumb-down.svg |
| assets/icons/cash.png | cash | lorc | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/lorc/cash.svg |
| assets/icons/hourglass.png | hourglass | lorc | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/lorc/hourglass.svg |
| assets/icons/backlog.png | stairs-goal | delapouite | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/delapouite/stairs-goal.svg |
| assets/icons/coins.png | coins-pile | delapouite | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/delapouite/coins-pile.svg |
| assets/icons/matrix.png | tic-tac-toe | delapouite | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/delapouite/tic-tac-toe.svg |
| assets/icons/key.png | key | lorc | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/lorc/key.svg |
| assets/icons/robot_audit.png | robot-golem | lorc | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/lorc/robot-golem.svg |
| assets/icons/padlock.png | padlock | lorc | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/lorc/padlock.svg |
| assets/icons/medal.png | medal | lorc | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/lorc/medal.svg |
| assets/icons/org_chart.png | organigram | delapouite | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/delapouite/organigram.svg |
| assets/icons/vibes.png | satellite-communication | delapouite | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/delapouite/satellite-communication.svg |
| assets/icons/parachute.png | parachute | lorc | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/lorc/parachute.svg |
| assets/icons/robot.png | robot-grab | delapouite | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/delapouite/robot-grab.svg |
| assets/icons/rss.png | rss | lorc | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/lorc/rss.svg |
| assets/icons/access_badge.png | police-badge | andymeneely | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/andymeneely/police-badge.svg |
| assets/icons/delivery_receipt.png | door | delapouite | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/delapouite/door.svg |
| assets/icons/scooter_helmet.png | full-motorcycle-helmet | delapouite | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/delapouite/full-motorcycle-helmet.svg |
| assets/icons/oat_milk_prompt.png | milk-carton | rihlsul | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/rihlsul/milk-carton.svg |
| assets/icons/stress_ball.png | throwing-ball | delapouite | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/delapouite/throwing-ball.svg |
| assets/icons/gpu_candle.png | candle-flame | lorc | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/lorc/candle-flame.svg |
| assets/icons/rack_screw.png | screw | delapouite | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/delapouite/screw.svg |
| assets/icons/token_invoice.png | price-tag | delapouite | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/delapouite/price-tag.svg |
| assets/icons/agent_memory.png | microchip | lorc | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/lorc/microchip.svg |
| assets/icons/cron.png | clockwork | lorc | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/lorc/clockwork.svg |
| assets/icons/queue_ticket.png | human-pyramid | delapouite | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/delapouite/human-pyramid.svg |
| assets/icons/priority_low.png | snail | lorc | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/lorc/snail.svg |
| assets/icons/thermal_paste.png | dripping-tube | lorc | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/lorc/dripping-tube.svg |
| assets/icons/incident_report.png | thermometer-hot | delapouite | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/delapouite/thermometer-hot.svg |
| assets/icons/fan_blade.png | computer-fan | delapouite | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/delapouite/computer-fan.svg |
| assets/icons/demo_laptop.png | laptop | delapouite | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/delapouite/laptop.svg |
| assets/icons/term_sheet.png | scroll-quill | delapouite | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/delapouite/scroll-quill.svg |
| assets/icons/prompt_certificate.png | ribbon-medal | delapouite | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/delapouite/ribbon-medal.svg |
| assets/icons/rack_chair.png | server-rack | delapouite | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/delapouite/server-rack.svg |
| assets/icons/still_typing.png | discussion | delapouite | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/delapouite/discussion.svg |
| assets/icons/benchmark.png | podium-winner | delapouite | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/delapouite/podium-winner.svg |
| assets/icons/visitor_badge.png | mug-shot | delapouite | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/delapouite/mug-shot.svg |
| assets/icons/judgment.png | semi-closed-eye | lorc | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/lorc/semi-closed-eye.svg |
| assets/icons/lighter.png | lighter | delapouite | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/delapouite/lighter.svg |
| assets/icons/room_booking.png | checklist | delapouite | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/delapouite/checklist.svg |
| assets/icons/wont_fix.png | cancel | sbed | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/sbed/cancel.svg |
| assets/icons/human_inside.png | person | delapouite | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/delapouite/person.svg |
| assets/icons/demo_lanyard.png | ribbon | lorc | CC BY 3.0 | https://github.com/game-icons/icons/blob/master/lorc/ribbon.svg |

Authors: Lorc (https://lorcblog.blogspot.com), Delapouite (https://delapouite.com), Andy Meneely (http://www.se.rit.edu/~andy/), sbed (http://opengameart.org/content/95-game-icons), rihlsul (https://github.com/game-icons/icons/tree/master/rihlsul). Glyph names and authors per https://game-icons.net/about.html.

The 28 item icons added later are regenerated by `scripts/game_icons.py`, which holds the glyph table.

## Drawn for this project

assets/icons/coffee_break.png is original art drawn in code by `scripts/coffee_icon.py` (the navy mug from the 3D model on a dark field), CC0. No external source.

## Procedural textures

The carpet, marble, ceiling tiles, posters, signs, Gantt charts, skyline and screens are generated in code
(`crates/game_client/src/floors/textures.rs`), text included, with a built-in 5x7 bitmap font. No external art or font is used for them.

## Fonts

| File | Font | Licence | Source |
|---|---|---|---|
| assets/fonts/JetBrainsMono-Bold.ttf | JetBrains Mono Bold v2.304, (c) 2020 The JetBrains Mono Project Authors | SIL Open Font License 1.1 (assets/fonts/OFL-JetBrainsMono.txt) | https://github.com/JetBrains/JetBrainsMono |

Icons whose key has no file in `icons/` are generated at runtime as letter badges (no external art).

## Audio

All audio is CC0 (https://creativecommons.org/publicdomain/zero/1.0/); attribution is not required but given gladly. Clips were converted to mono Ogg Vorbis, trimmed or layered with ffmpeg; nothing is AI-generated. assets/audio/LICENSE-kenney.txt is the original Kenney licence text.

| Used for | Source | Author | Licence |
|---|---|---|---|
| Interface and pop-ups (error, spell learned, elevator ding, summon, heal, tick, enemy cast) | Interface Sounds, https://kenney.nl/assets/interface-sounds | Kenney | CC0 |
| Cast zaps, tones and phaser sweeps (Code Review, git blame, Rollback, Refactor, death) | Digital Audio, https://kenney.nl/assets/digital-audio | Kenney | CC0 |
| Hits, stamp thud, bonk, crit bell, Postmortem, Write It By Hand impact | Impact Sounds, https://kenney.nl/assets/impact-sounds | Kenney | CC0 |
| Coins (loot), trophy window book open / close | RPG Audio, https://kenney.nl/assets/rpg-audio | Kenney | CC0 |
| Elevator door, Guardrails and cast hum shield tones | Sci-Fi Sounds, https://kenney.nl/assets/sci-fi-sounds | Kenney | CC0 |
| Level-up, floor-cleared and victory jingles | Music Jingles, https://kenney.nl/assets/music-jingles | Kenney | CC0 |
| Keycap clacks, typing burst | Keyboard Soundpack 1, https://opengameart.org/content/keyboard-soundpack-1-typing-and-single-keystrokes | unicaegames | CC0 |
| Rubber Duck squeak | Squeak toy sounds, https://opengameart.org/content/squeak-toy-sounds | bart | CC0 |
| Caffeine and Coffee Break sips | Liquid / bottle / drink set, https://opengameart.org/content/liquid-bottle-drink-set | qubodup | CC0 |
| Miss and Rollback whooshes | Swishes sound pack, https://opengameart.org/content/swishes-sound-pack | artisticdude | CC0 |
| Air-con ambience (low-passed) | 30 CC0 SFX loops, https://opengameart.org/content/30-cc0-sfx-loops | rubberduck | CC0 |
| Exploration music (assets/audio/music/office_muzak.ogg) | Elevator Music, https://opengameart.org/content/elevator-music | Alex McCulloch (Pro Sensory) | CC0 |
| Boss music (assets/audio/music/boss_battle.ogg) | Boss Battle Music (Epic Boss Battle, seamlessly looping), https://opengameart.org/content/boss-battle-music | Juhani Junkala (uploaded by SubspaceAudio) | CC0 |
