import js from '@eslint/js'
import globals from 'globals'
import reactHooks from 'eslint-plugin-react-hooks'
import reactRefresh from 'eslint-plugin-react-refresh'
import tseslint from 'typescript-eslint'

export default tseslint.config(
  { ignores: ['dist', 'node_modules'] },
  {
    extends: [js.configs.recommended, ...tseslint.configs.recommended],
    files: ['**/*.{ts,tsx}'],
    languageOptions: {
      ecmaVersion: 2020,
      globals: globals.browser,
    },
    plugins: {
      'react-hooks': reactHooks,
      'react-refresh': reactRefresh,
    },
    rules: {
      ...reactHooks.configs.recommended.rules,
      'react-refresh/only-export-components': [
        'warn',
        { allowConstantExport: true },
      ],
    },
  },
  // Context files intentionally co-export the Provider component, the hook,
  // and any utility functions (e.g. applyInitialThemeSync). Suppressing the
  // react-refresh warning here is correct — these files aren't hot-module
  // boundaries in the way a feature component file is.
  {
    files: ['src/**/*Context.tsx'],
    rules: {
      'react-refresh/only-export-components': 'off',
    },
  },
)
