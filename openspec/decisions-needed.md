# Decisions needed

Open questions that need a human. One heading per decision, each with the
options and a recommended answer. Remove an entry once it is decided and
recorded (ADR, spec or issue).

## Decide-role model ("laya")

The brief named a model for the `decide` role that reads as "laya". We assume
it means the **Llama** family; the catalog ships Llama 3.1 8B Instruct as one
`decide` suggestion, alongside Qwen3 8B and Mistral Small. Any model is
configurable regardless.
**Recommended:** confirm the intended model, or let the AI-gateway benchmark
task pick the default by measured accuracy on our classification set.

## Platform direction (custom models + workflows + dashboards)

Whether Campbooks should become a platform (user-defined models, workflow
automation over any model, a dashboard builder) with mail, calendar, tasks,
files and accounting as built-in modules on the same primitives.
**Recommended:** decide as part of the domain-mapping task, before any domain
module beyond `system` is specced; it changes how modules store data.
