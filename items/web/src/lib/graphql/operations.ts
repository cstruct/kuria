import { graphql } from './generated';

/** The changes the app follows; Graphcache's update marks what is stale. */
export const Changes = graphql(`
	subscription Changes {
		changes {
			seq
			kind
			akt
			number
		}
	}
`);

export const Home = graphql(`
	query Home {
		viewer {
			subject
			tenant
		}
		aktTypes {
			key
			version
			name
		}
	}
`);

export const AkterOfType = graphql(`
	query AkterOfType($type: String!) {
		aktType(key: $type) {
			key
			version
			name
		}
		akter(type: $type, first: 200) {
			id
			number
			state {
				key
				name
			}
		}
	}
`);
