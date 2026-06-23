# Product

## Register

product

## Users

Young, AI-native professionals early in their careers (roughly 22 to 30). They grew up on ChatGPT, Notion, Linear, Arc, Discord, and consumer apps that feel alive and responsive. They are now stepping into professional settings where email is unavoidable, and they find legacy email (Gmail, Outlook) static, dull, and beneath the standard every other app in their life has set.

They expect AI to do the grunt work by default, not as an opt-in feature. They expect the interface to be dynamic, responsive, and genuinely enjoyable. Crucially, they do **not** want to "get in and get out": they are happy to spend time inside a tool that makes the work feel good, as long as it hands the boredom to the AI.

This audience is chosen deliberately and narrowly. Campbooks does **not** chase broad, mass-market, "cater to everyone" appeal. It commits to this person and designs for their taste, even when that means the bolder choice over the safe one.

## Product Purpose

Campbooks ingests email and documents and uses AI to do the boring, repetitive work: sorting, reading, summarizing, drafting, and surfacing what actually matters. What is left for the human is the rewarding part: deciding, conversing with the AI assistant (Scout), and moving through a fast, satisfying, dynamic triage flow (Skim) whose value is always knowing what has been addressed and what has not.

Success is not "inbox zero, then leave." Success is: the user genuinely enjoys opening Campbooks, spends their attention on judgment and insight rather than drudgery, and feels like they are using something from the near future rather than from 2012.

## Brand Personality

Dynamic, expressive, AI-native, confident. Alive rather than calm-by-default. It carries the visual confidence and warmth of a great consumer app, applied to a serious professional tool. It is satisfying and a little playful to use without being a toy, and bold without being loud for its own sake.

It treats the AI (Scout) as a real, present collaborator woven through the product, never a bolted-on sidebar. It celebrates momentum and progress. It never feels like enterprise software, and never like a sterile productivity utility.

## Anti-references

- **Outlook / Gmail**: static lists, toolbar overload, corporate dullness. "Email from 2012." The thing this audience is fleeing.
- **Generic SaaS / default shadcn**: the "sea of sameness," the Linear-clone look where every B2B app is interchangeable. Distinctive by refusing the template.
- **HEY (Basecamp)**: austere, preachy, opinionated-minimalist, text-wall. Personality delivered by lecturing the user instead of by the experience. Explicitly disliked.
- **Superhuman**: cold, keyboard-monk minimalism engineered to get you *out* fast. Wrong thesis: we want people to enjoy being *in*.
- **Fintech sterility**: cold whites, navy-and-gold, joyless and transactional.
- **"Cater to all" blandness**: designing for the median user, committing to no one, and landing back in the sea of sameness from the other direction.

## Design Principles

1. **AI does the boring part, visibly.** The interface should constantly show that the drudgery is handled: sorting, summaries, drafts, triage. The human's attention goes to judgment, not janitorial work. Scout and AI output are protagonists woven through every surface, not an afterthought.

2. **Dynamic, not static.** Email should feel alive: motion, responsiveness, momentum, state that updates and reacts in front of you. Reject the static three-pane list as the only model. Movement is meaningful, never decorative.

3. **Make the work feel good.** Borrow the satisfaction of consumer apps: tactile interactions, momentum, the "feels good to clear through this" quality of Skim. Earn time-in-app through delight and value. Engagement is a legitimate goal; genuine dark patterns (FOMO counters, manufactured streaks, vanishing content, social comparison) are flagged for explicit human sign-off and are never the default.

4. **Expressive through color and motion, calm in type.** The surface stays clean and near-neutral (whisper-warm) so the content pops, Instagram-style, never drenched in a brand hue. Expression is carried by three things, never by a decorative font: a single warm signature gradient ("Ember", a magenta-to-orange sunset) reserved for special accents (Scout, the Skim flow, moments of delight, the key call to action); near-black as the functional color for primary actions, active states, and emphasis (it replaced the old electric-violet, which is retired); and motion. Type in the **product UI is Inter only**, body and headings alike, with hierarchy built from weight and scale and no display or grotesk face anywhere inside the app. Expressive display faces (Fraunces, Clash, other grotesques) live **only in the brand and marketing layer**, the website, posters, logo, and campaigns, never in the UI. The functional status palette (green/amber/red/blue) carries state and nothing else. Depth and glow are part of the language, strongest on the dark theme where Ember glows.

5. **Sharp audience focus.** Design for the young, AI-native professional, not the median user. When in doubt, choose the bolder, more dynamic option that this person expects over the safe one that offends no one.

6. **Bold by default, with an escape hatch.** Dynamic and expressive is the baseline. Power users (and dense-data moments) can drop to a familiar, compact mode. Boldness is the default state, never a cage.

## Accessibility & Inclusion

WCAG 2.1 AA minimum. Sufficient color contrast even with a Committed, saturated palette. Keyboard-navigable, screen-reader friendly, visible focus indicators. Status and state communicated through more than color or motion alone. Motion-forward design must fully honor `prefers-reduced-motion`: when reduced motion is requested, the product stays legible and usable with movement removed, never broken.
