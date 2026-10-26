## ADDED Requirements

### Requirement: The design system is a shared package
The design system SHALL live in the package `@notacamp/ui` (`clients/ui`):
the tokens and themes, the generic components with their stories and tests,
the keyboard shortcut engine (`@notacamp/ui/shortcuts`) and their strings in
every shipped locale. The package SHALL import nothing from an app; a
component that names a Campbooks concept SHALL stay in the app, built from
the package's components. The app SHALL use the package from its source
through `~/lib/ui` and `~/lib/shortcuts`, which add its own components, its
shortcut catalogue, its preferences store and its strings. Themes SHALL be
token sets: an app SHALL be able to give the components its own look by
supplying its own values for the package's tokens, without changing a
component. The package SHALL be published to GitHub Packages from a
`ui-v<version>` tag on `next` whose version is the package's.

#### Scenario: The package reaching into the app
- **WHEN** a file in `clients/ui` imports from `~/` or from the app
- **THEN** `pnpm lint` and `pnpm deps:check` fail

#### Scenario: Campbooks without a package build
- **WHEN** the app is started, tested or built with nothing built in `clients/ui`
- **THEN** it uses the package's source, and its screens, strings, shortcuts and saved theme are the same as before the package existed

#### Scenario: Another app's look
- **WHEN** an app imports its own token set after the package's `tokens.css`
- **THEN** every component draws with that app's values, and no component is changed

#### Scenario: A tag that is not the version
- **WHEN** `ui-v0.2.0` is pushed while the package's version is 0.1.0, or the tagged commit is not on `next`
- **THEN** the publish workflow fails before building and nothing is published

## MODIFIED Requirements

### Requirement: Token-only styling is enforced
CI SHALL fail when CSS outside the token files contains a colour, size,
duration or easing literal, when a class name uses a Tailwind arbitrary
value or a palette colour, when a module uses an inline style or a
non-layout utility, or when code outside the design system imports anything
but its public index. The app's design system is `@notacamp/ui` (`clients/ui`)
plus the app's own components in `src/lib/ui/`, and its only entry point for
modules and the app shell is `src/lib/ui/index.ts`, which re-exports the
package.

#### Scenario: A literal colour in a component
- **WHEN** a component stylesheet declares `color: #123456`
- **THEN** `pnpm lint` fails

#### Scenario: A module reaching into a component
- **WHEN** a module imports a file under `src/lib/ui/` other than `index.ts`
- **THEN** `pnpm deps:check` fails

#### Scenario: A module importing the package directly
- **WHEN** a module or the app shell imports `@notacamp/ui` or a file under `clients/ui` instead of `~/lib/ui`
- **THEN** `pnpm deps:check` fails
