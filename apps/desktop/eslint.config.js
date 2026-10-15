import js from "@eslint/js";
import tseslint from "typescript-eslint";
import reactHooks from "eslint-plugin-react-hooks";
import i18next from "eslint-plugin-i18next";

/** Physical left/right utilities; use the logical ones (ms-/me-, ps-/pe-, start-/end-, border-s/e, text-start/end). */
const PHYSICAL = String.raw`(^|\s)-?(ml|mr|pl|pr|left|right|border-[lr]|rounded-[lr]|rounded-[tb][lr]|text-left|text-right|float-left|float-right|scroll-m[lr]|scroll-p[lr])(-|\s|$)`;
const physicalMessage =
  "Use logical CSS utilities (ms-/me-, ps-/pe-, start-/end-, border-s/-e, rounded-s/-e, text-start/-end) so right-to-left languages mirror.";

export default tseslint.config(
  { ignores: ["dist", "src-tauri"] },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    plugins: { "react-hooks": reactHooks },
    rules: reactHooks.configs.recommended.rules,
  },
  {
    // Plain scripts loaded by index.html before the app.
    files: ["public/**/*.js"],
    languageOptions: { globals: { window: "readonly", document: "readonly", localStorage: "readonly" } },
  },
  {
    files: ["src/**/*.tsx"],
    ignores: ["src/**/*.test.tsx"],
    plugins: { i18next },
    rules: {
      // Every visible string goes through t(): JSX text and user-facing attributes.
      "i18next/no-literal-string": [
        "error",
        {
          mode: "jsx-only",
          "jsx-attributes": { include: ["alt", "aria-label", "aria-description", "title", "placeholder", "label"] },
          words: {
            exclude: [
              // Punctuation and separators between translated parts.
              String.raw`^[\s.,:;·—–\-()/|•]*$`,
              // Product names are never translated.
              String.raw`^(nginx|Apache|MySQL|MariaDB|PHP|Redis|Node\.js|Composer|ngrok|Lumina)$`,
            ],
          },
        },
      ],
      "no-restricted-syntax": [
        "error",
        { selector: `JSXAttribute[name.name='className'] Literal[value=/${PHYSICAL}/]`, message: physicalMessage },
        { selector: `JSXAttribute[name.name='className'] TemplateElement[value.raw=/${PHYSICAL}/]`, message: physicalMessage },
      ],
    },
  },
);
