//! Actions: what a module lets a caller *do*, declared once with a typed
//! builder and turned into an HTTP endpoint, a catalogue entry and (later) an
//! MCP tool, an assistant tool, an automation step and a command-palette
//! entry.

use std::{future::Future, marker::PhantomData, pin::Pin, sync::Arc, time::Duration};

use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use utoipa::ToSchema;

use super::{ActionContext, SchemaDecl};
use crate::{Error, Result, auth::ScopeName};

/// How long an undo token stays redeemable unless the action says otherwise
/// ([`Action::undo_window`]).
pub const DEFAULT_UNDO_WINDOW: Duration = Duration::from_mins(10);

type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;
type RunFn = Arc<dyn Fn(ActionContext, Value) -> BoxFuture<Result<Outcome>> + Send + Sync>;
type UndoFn = Arc<dyn Fn(ActionContext, Value) -> BoxFuture<Result<()>> + Send + Sync>;

/// What a type-erased handler produced: its output, and the undo payload if
/// there is something to undo.
pub(crate) struct Outcome {
    pub output: Value,
    pub undo: Option<Value>,
}

/// An action's builder. Start with [`Action::new`], finish with
/// [`handler`](Action::handler), which yields the [`ActionDecl`] a module
/// lists in its [`Declarations`](super::Declarations):
///
/// ```ignore
/// pub fn action() -> ActionDecl {
///     Action::<ArchiveInput, ArchiveOutput>::new("conversations.archive")
///         .describe("Archive a conversation: it leaves the inbox until something new arrives.")
///         .scope::<ConversationsWrite>()
///         .undoable(unarchive)
///         .shortcut("e")
///         .handler(archive)
/// }
/// ```
///
/// - `I` is the input (the request body), `O` the output (`result` in the
///   response); both derive `Serialize`, `Deserialize`/`Serialize` and
///   `ToSchema`.
/// - [`scope`](Self::scope) is required: `handler` only exists once the
///   action has one.
/// - [`undoable`](Self::undoable) changes what the handler returns to
///   [`Undoable<O, U>`], where `U` is what the undo handler needs.
#[must_use = "an action does nothing until `.handler(…)` turns it into an `ActionDecl`"]
pub struct Action<I, O, S = NoScope, U = NoUndo> {
    name: &'static str,
    description: &'static str,
    label: Option<&'static str>,
    mutates: bool,
    shortcut: Option<&'static str>,
    undo: U,
    types: PhantomData<fn(I, S) -> O>,
}

/// Type state: no scope chosen yet.
#[derive(Debug, Clone, Copy)]
pub struct NoScope;

/// Type state: the action cannot be undone.
#[derive(Debug, Clone, Copy)]
pub struct NoUndo;

/// Type state: the action can be undone with payload `U`.
pub struct WithUndo<U> {
    run: UndoFn,
    window: Duration,
    payload: PhantomData<fn() -> U>,
}

impl<I, O> Action<I, O, NoScope, NoUndo> {
    /// An action named `<module>.<verb>` (lower-case snake case, e.g.
    /// `conversations.archive`). The name is a wire contract — its URL is
    /// `POST /api/v1/actions/<name>` — so never rename it.
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            description: "",
            label: None,
            mutates: false,
            shortcut: None,
            undo: NoUndo,
            types: PhantomData,
        }
    }
}

impl<I, O, S, U> Action<I, O, S, U> {
    /// What the action does, in one or two plain sentences (English). It is
    /// the operation's description in the OpenAPI document and what an
    /// assistant or MCP client reads to decide when to call it. Required.
    pub fn describe(mut self, description: &'static str) -> Self {
        self.description = description;
        self
    }

    /// The scope a caller's token must grant, declared in the module's
    /// `Module.scopes`.
    pub fn scope<S2: ScopeName>(self) -> Action<I, O, S2, U> {
        Action {
            name: self.name,
            description: self.description,
            label: self.label,
            mutates: self.mutates,
            shortcut: self.shortcut,
            undo: self.undo,
            types: PhantomData,
        }
    }

    /// The action changes data. Mutating actions honour `Idempotency-Key`,
    /// and clients confirm or offer undo for them. Implied by
    /// [`undoable`](Self::undoable).
    pub fn mutates(mut self) -> Self {
        self.mutates = true;
        self
    }

    /// The default keyboard shortcut clients bind to the action: chords
    /// separated by single spaces, each `[mod+][ctrl+][alt+][shift+]<key>`,
    /// where `mod` is ⌘ on Apple platforms and Ctrl elsewhere, and `<key>`
    /// is a lower-case letter, a digit, a punctuation character or a named
    /// key (`enter`, `escape`, `up`, `f1`, …). E.g. `"e"`, `"g i"`,
    /// `"mod+enter"`. Unique within the module.
    pub fn shortcut(mut self, shortcut: &'static str) -> Self {
        self.shortcut = Some(shortcut);
        self
    }

    /// The i18n key of the action's label, when not the default
    /// `<module>:actions.<verb>` (an i18next `namespace:key`).
    pub fn label(mut self, key: &'static str) -> Self {
        self.label = Some(key);
        self
    }
}

impl<I, O, S> Action<I, O, S, NoUndo> {
    /// The action can be undone by `undo`, which receives the payload the
    /// handler returned in [`Undoable`]. The response then carries an undo
    /// token, redeemable once at `POST /api/v1/actions/undo` by the same
    /// caller within the [undo window](Self::undo_window). Implies
    /// [`mutates`](Self::mutates).
    ///
    /// The payload is stored until the token expires: keep it to ids and the
    /// prior state the undo needs, never secrets or message bodies.
    pub fn undoable<U, F, Fut>(self, undo: F) -> Action<I, O, S, WithUndo<U>>
    where
        U: DeserializeOwned + Send + 'static,
        F: Fn(ActionContext, U) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        let undo = Arc::new(undo);
        let run: UndoFn = Arc::new(move |ctx, payload| {
            let undo = Arc::clone(&undo);
            Box::pin(async move {
                let payload: U = serde_json::from_value(payload).map_err(Error::internal)?;
                undo(ctx, payload).await
            })
        });
        Action {
            name: self.name,
            description: self.description,
            label: self.label,
            mutates: true,
            shortcut: self.shortcut,
            undo: WithUndo {
                run,
                window: DEFAULT_UNDO_WINDOW,
                payload: PhantomData,
            },
            types: PhantomData,
        }
    }
}

impl<I, O, S, U> Action<I, O, S, WithUndo<U>> {
    /// How long the undo token stays redeemable (default
    /// [`DEFAULT_UNDO_WINDOW`]).
    pub fn undo_window(mut self, window: Duration) -> Self {
        self.undo.window = window;
        self
    }
}

impl<I, O, S> Action<I, O, S, NoUndo>
where
    I: Serialize + DeserializeOwned + ToSchema + Send + 'static,
    O: Serialize + ToSchema + Send + 'static,
    S: ScopeName,
{
    /// The function that runs the action:
    /// `async fn(ActionContext, I) -> Result<O>`.
    pub fn handler<F, Fut>(self, handler: F) -> ActionDecl
    where
        F: Fn(ActionContext, I) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<O>> + Send + 'static,
    {
        let handler = Arc::new(handler);
        let run: RunFn = Arc::new(move |ctx, input| {
            let handler = Arc::clone(&handler);
            Box::pin(async move {
                let output = handler(ctx, decode::<I>(input)?).await?;
                Ok(Outcome {
                    output: serde_json::to_value(output).map_err(Error::internal)?,
                    undo: None,
                })
            })
        });
        self.finish::<I, O>(run, None)
    }
}

impl<I, O, S, U> Action<I, O, S, WithUndo<U>>
where
    I: Serialize + DeserializeOwned + ToSchema + Send + 'static,
    O: Serialize + ToSchema + Send + 'static,
    S: ScopeName,
    U: Serialize + Send + 'static,
{
    /// The function that runs the action:
    /// `async fn(ActionContext, I) -> Result<Undoable<O, U>>`.
    pub fn handler<F, Fut>(self, handler: F) -> ActionDecl
    where
        F: Fn(ActionContext, I) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Undoable<O, U>>> + Send + 'static,
    {
        let handler = Arc::new(handler);
        let run: RunFn = Arc::new(move |ctx, input| {
            let handler = Arc::clone(&handler);
            Box::pin(async move {
                let Undoable { output, undo } = handler(ctx, decode::<I>(input)?).await?;
                Ok(Outcome {
                    output: serde_json::to_value(output).map_err(Error::internal)?,
                    undo: undo
                        .map(serde_json::to_value)
                        .transpose()
                        .map_err(Error::internal)?,
                })
            })
        });
        let undo = UndoDecl {
            window: self.undo.window,
            run: Arc::clone(&self.undo.run),
        };
        self.finish::<I, O>(run, Some(undo))
    }
}

impl<I, O, S: ScopeName, U> Action<I, O, S, U> {
    fn finish<In, Out>(self, run: RunFn, undo: Option<UndoDecl>) -> ActionDecl
    where
        In: Serialize + DeserializeOwned + ToSchema,
        Out: ToSchema,
    {
        ActionDecl {
            name: self.name,
            description: self.description,
            label: self.label,
            scope: S::NAME,
            mutates: self.mutates,
            shortcut: self.shortcut,
            input: SchemaDecl::of::<In>(),
            output: SchemaDecl::of::<Out>(),
            normalize: normalize::<In>,
            run,
            undo,
        }
    }
}

/// An action's output, and what its undo handler needs to reverse it —
/// `None` when the call changed nothing (archiving an archived
/// conversation), in which case the response carries no undo token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Undoable<O, U> {
    output: O,
    undo: Option<U>,
}

impl<O, U> Undoable<O, U> {
    /// `output`, undone by running the undo handler with `undo`.
    pub fn new(output: O, undo: U) -> Self {
        Self {
            output,
            undo: Some(undo),
        }
    }

    /// `output`, with nothing to undo.
    pub fn nothing_to_undo(output: O) -> Self {
        Self { output, undo: None }
    }
}

/// The input `I` decoded from JSON; a mismatch is the caller's fault (422).
fn decode<I: DeserializeOwned>(input: Value) -> Result<I> {
    serde_json::from_value(input).map_err(|e| Error::Unprocessable(e.to_string()))
}

/// The input decoded as `I` and encoded again: validates it and gives equal
/// inputs one canonical form (the idempotency fingerprint is taken over it).
fn normalize<I: Serialize + DeserializeOwned>(input: Value) -> Result<Value> {
    let input: I = decode(input)?;
    serde_json::to_value(input).map_err(Error::internal)
}

/// How an undoable action is undone.
#[derive(Clone)]
pub(crate) struct UndoDecl {
    pub window: Duration,
    pub run: UndoFn,
}

/// A declared action: the finished [`Action`] builder, type-erased so the
/// server can hold every module's actions in one registry.
#[derive(Clone)]
pub struct ActionDecl {
    name: &'static str,
    description: &'static str,
    label: Option<&'static str>,
    scope: &'static str,
    mutates: bool,
    shortcut: Option<&'static str>,
    pub(crate) input: SchemaDecl,
    pub(crate) output: SchemaDecl,
    pub(crate) normalize: fn(Value) -> Result<Value>,
    pub(crate) run: RunFn,
    pub(crate) undo: Option<UndoDecl>,
}

impl std::fmt::Debug for ActionDecl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActionDecl")
            .field("name", &self.name)
            .field("scope", &self.scope)
            .field("mutates", &self.mutates)
            .field("undoable", &self.undo.is_some())
            .field("shortcut", &self.shortcut)
            .finish_non_exhaustive()
    }
}

impl ActionDecl {
    /// Wire name, `<module>.<verb>`.
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// The module that declares it (the name's prefix).
    pub fn module(&self) -> &'static str {
        self.name
            .split_once('.')
            .map_or(self.name, |(module, _)| module)
    }

    /// What it does, for people reading the API and for assistants.
    pub fn description(&self) -> &'static str {
        self.description
    }

    /// The i18n key of its label.
    pub fn label(&self) -> String {
        self.label
            .map_or_else(|| super::label_key(self.name, "actions"), str::to_owned)
    }

    /// The scope a caller's token must grant.
    pub fn scope(&self) -> &'static str {
        self.scope
    }

    /// Whether it changes data.
    pub fn mutates(&self) -> bool {
        self.mutates
    }

    /// How long its undo tokens stay redeemable; `None` if it cannot be
    /// undone.
    pub fn undo_window(&self) -> Option<Duration> {
        self.undo.as_ref().map(|undo| undo.window)
    }

    /// Its default keyboard shortcut.
    pub fn shortcut(&self) -> Option<&'static str> {
        self.shortcut
    }

    /// Its endpoint, `/api/v1/actions/<name>`.
    pub fn path(&self) -> String {
        format!("{}/{}", super::http::ACTIONS_PATH, self.name)
    }

    /// Its OpenAPI operation id, `<module>_<verb>`.
    pub fn operation_id(&self) -> String {
        self.name.replace('.', "_")
    }

    /// The OpenAPI component name of its input.
    pub fn input_schema_name(&self) -> &str {
        &self.input.name
    }

    /// The OpenAPI component name of its output.
    pub fn output_schema_name(&self) -> &str {
        &self.output.name
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    struct TestWrite;
    impl ScopeName for TestWrite {
        const NAME: &'static str = "test:write";
        const DESCRIPTION: &'static str = "Change test things.";
    }

    #[derive(Debug, Serialize, Deserialize, ToSchema)]
    struct Input {
        n: u32,
    }

    #[derive(Debug, Serialize, ToSchema)]
    struct Output {
        doubled: u32,
    }

    #[derive(Serialize, Deserialize)]
    struct Restore {
        n: u32,
    }

    async fn double(_: ActionContext, input: Input) -> Result<Output> {
        Ok(Output {
            doubled: input.n * 2,
        })
    }

    async fn double_undoably(_: ActionContext, input: Input) -> Result<Undoable<Output, Restore>> {
        Ok(Undoable::new(
            Output {
                doubled: input.n * 2,
            },
            Restore { n: input.n },
        ))
    }

    async fn restore(_: ActionContext, _: Restore) -> Result<()> {
        Ok(())
    }

    #[test]
    fn builder_records_what_was_declared() {
        let plain = Action::<Input, Output>::new("test.double")
            .describe("Double a number.")
            .scope::<TestWrite>()
            .shortcut("d")
            .handler(double);
        assert_eq!(plain.name(), "test.double");
        assert_eq!(plain.module(), "test");
        assert_eq!(plain.scope(), "test:write");
        assert!(!plain.mutates());
        assert_eq!(plain.undo_window(), None);
        assert_eq!(plain.shortcut(), Some("d"));
        assert_eq!(plain.label(), "test:actions.double");
        assert_eq!(plain.path(), "/api/v1/actions/test.double");
        assert_eq!(plain.operation_id(), "test_double");
        assert_eq!(plain.input_schema_name(), "Input");
        assert_eq!(plain.output_schema_name(), "Output");

        let undoable = Action::<Input, Output>::new("test.double_undoably")
            .describe("Double a number, undoably.")
            .scope::<TestWrite>()
            .label("test:actions.custom")
            .undoable(restore)
            .undo_window(Duration::from_secs(30))
            .handler(double_undoably);
        assert!(undoable.mutates(), "undoable implies mutates");
        assert_eq!(undoable.undo_window(), Some(Duration::from_secs(30)));
        assert_eq!(undoable.label(), "test:actions.custom");
    }

    #[test]
    fn normalize_validates_and_canonicalizes() {
        let normalized = normalize::<Input>(serde_json::json!({"n": 2, "extra": true})).unwrap();
        assert_eq!(normalized, serde_json::json!({"n": 2}));
        let Err(Error::Unprocessable(message)) = normalize::<Input>(serde_json::json!({})) else {
            panic!("a missing field is unprocessable");
        };
        assert!(message.contains("missing field `n`"), "{message}");
    }
}
