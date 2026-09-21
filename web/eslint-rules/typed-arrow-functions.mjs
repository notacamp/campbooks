/**
 * typed-arrow-functions — Campbooks re-implementation.
 *
 * Module-level functions must be typed arrow constants. Two checks:
 *   1. A `function X() {}` at module level → convert to `const X = () => {}`
 *   2. A `const X = () => {}` with no type on the constant OR the arrow's
 *      return → annotate it. Not auto-fixable (the type is a decision).
 *
 * Exempt: generators, overloaded functions (paired TSDeclareFunction siblings),
 * `declare function`, anonymous `export default function`. Nested helpers inside
 * a body are not this rule's concern.
 *
 * Only applies to TypeScript files; .mjs / .cjs script files carry no type
 * syntax and should be excluded via the ESLint config's `files` glob.
 */

/**
 * Returns true when the parent node is the module's program body, or when it is
 * a named/default export wrapping at that top level.
 */
function isModuleLevel(parent) {
  if (parent.type === "Program") return true;
  if (
    (parent.type === "ExportNamedDeclaration" ||
      parent.type === "ExportDefaultDeclaration") &&
    parent.parent.type === "Program"
  ) {
    return true;
  }
  return false;
}

/**
 * True when the function has sibling TSDeclareFunction entries that share its
 * name — i.e. it is an overloaded implementation.
 */
function hasOverloads(node) {
  const container =
    node.parent.type === "Program" ? node.parent.body : node.parent.parent.body;
  if (!Array.isArray(container)) return false;
  return container.some((sibling) => {
    const decl =
      sibling.type === "ExportNamedDeclaration" ? sibling.declaration : sibling;
    return (
      decl &&
      decl.type === "TSDeclareFunction" &&
      decl.id &&
      node.id &&
      decl.id.name === node.id.name
    );
  });
}

/**
 * Builds the arrow-function replacement text for a FunctionDeclaration node.
 * Handles async, generics, parameters, return type, and body.
 */
function arrowTextFor(node, sourceCode) {
  const name = node.id.name;
  const asyncPrefix = node.async ? "async " : "";

  let typeParams = "";
  if (node.typeParameters) {
    typeParams = sourceCode.getText(node.typeParameters);
    // A single generic `<T>` in a .tsx file looks like a JSX tag. Add a
    // trailing comma to disambiguate: `<T,>` is unambiguous in both .ts/.tsx.
    if (
      node.typeParameters.params.length === 1 &&
      !typeParams.includes(",")
    ) {
      typeParams = typeParams.replace(/>$/, ",>");
    }
  }

  const parenOpen = sourceCode.getTokenAfter(
    node.typeParameters ?? node.id,
    (token) => token.value === "(",
  );
  const parenClose = sourceCode.getTokenBefore(
    node.returnType ?? node.body,
    (token) => token.value === ")",
  );
  const params = sourceCode.text.slice(
    parenOpen.range[0],
    parenClose.range[1],
  );
  const returnType = node.returnType ? sourceCode.getText(node.returnType) : "";
  const body = sourceCode.getText(node.body);

  return `const ${name} = ${asyncPrefix}${typeParams}${params}${returnType} => ${body};`;
}

export default {
  meta: {
    type: "suggestion",
    docs: {
      description:
        "Module-level functions must be typed arrow constants: `const X: Type = () => …` or `const X = (): Return => …`.",
    },
    fixable: "code",
    schema: [],
    messages: {
      declaration:
        "Declare {{name}} as an arrow constant (`const {{name}} = () => …`). Annotate the type on the constant or its return.",
      untyped:
        "{{name}} needs a type: annotate the constant (`const {{name}}: FC<Props> = …`) or the arrow's return (`(): ReturnType => …`).",
    },
  },

  create(context) {
    const sourceCode = context.sourceCode;

    return {
      FunctionDeclaration(node) {
        if (!isModuleLevel(node.parent)) return;
        if (!node.id) return; // anonymous (impossible for FunctionDeclaration, but safe)
        if (node.generator) return;
        if (node.declare) return;
        if (hasOverloads(node)) return;

        // Named `export default function Foo()` is reportable but cannot be
        // auto-fixed: `export default const` is not valid syntax.
        const fixable = node.parent.type !== "ExportDefaultDeclaration";

        context.report({
          node: node.id,
          messageId: "declaration",
          data: { name: node.id.name },
          fix: fixable
            ? (fixer) =>
                fixer.replaceText(node, arrowTextFor(node, sourceCode))
            : null,
        });
      },

      VariableDeclarator(node) {
        const { init, id, parent } = node;
        if (!init) return;
        if (
          init.type !== "ArrowFunctionExpression" &&
          init.type !== "FunctionExpression"
        ) {
          return;
        }
        if (init.generator) return;
        if (!isModuleLevel(parent.parent)) return;
        if (id.type !== "Identifier") return;
        // Already typed: either the binding or the function return carry a
        // type annotation — nothing to report.
        if (id.typeAnnotation || init.returnType) return;

        context.report({
          node: id,
          messageId: "untyped",
          data: { name: id.name },
        });
      },
    };
  },
};
