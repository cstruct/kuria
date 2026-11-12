import type { CodegenConfig } from '@graphql-codegen/cli';

/**
 * Typed operations from the server's schema (`schema.graphql`, printed by
 * `kuria-server schema`), and its introspection for Graphcache, which needs
 * to know the unions' members.
 */
const config: CodegenConfig = {
	schema: 'schema.graphql',
	// Operations live in .ts modules, never in components.
	documents: ['src/**/*.ts', '!src/lib/graphql/generated/**'],
	ignoreNoDocuments: true,
	generates: {
		'src/lib/graphql/generated/': {
			preset: 'client',
			config: {
				useTypeImports: true,
				enumsAsTypes: true,
				scalars: { DateTime: 'string', JSON: 'unknown', ID: 'string' }
			},
			presetConfig: { fragmentMasking: false }
		},
		'src/lib/graphql/generated/introspection.json': {
			plugins: ['introspection'],
			config: { minify: true, descriptions: false }
		}
	}
};

export default config;
