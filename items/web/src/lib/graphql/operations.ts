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
			akterCount
		}
		akter(first: 10) {
			id
			number
			title
			updatedAt
			state {
				key
				name
				tone
			}
			type {
				key
				version
				name
			}
		}
	}
`);

export const AkterOfType = graphql(`
	query AkterOfType($type: String!, $state: String) {
		aktType(key: $type) {
			...ShownType
			states {
				key
				name
			}
		}
		akter(type: $type, state: $state, first: 200) {
			id
			number
			title
			rank
			updatedAt
			state {
				key
				name
				tone
			}
			type {
				...ShownType
			}
			fields {
				...FieldValues
			}
		}
	}
`);

export const FieldValues = graphql(`
	fragment FieldValues on FieldValue {
		__typename
		... on TextValue {
			field
			text
		}
		... on ChoiceValue {
			field
			option {
				key
				name
			}
		}
		... on ChoicesValue {
			field
			options {
				key
				name
			}
		}
		... on DateValue {
			field
			date
		}
		... on UserValue {
			field
			user {
				...SomeoneShown
			}
		}
		... on CheckboxValue {
			field
			checked
			at
			by {
				id
				name
			}
			for {
				...SomeoneShown
			}
		}
		... on ReferenceValue {
			field
			url
			label
		}
		... on GitValue {
			field
			repository
			sha
			tag
		}
		... on DockerValue {
			field
			repository
			digest
			tag
		}
		... on AttachmentValue {
			field
			attachment {
				id
				url
				name
				mediaType
				size
				sbom {
					format
					specVersion
					name
					components
				}
			}
		}
	}
`);

export const SomeoneShown = graphql(`
	fragment SomeoneShown on Someone {
		__typename
		... on User {
			id
			name
		}
		... on Contact {
			id
			displayName
		}
	}
`);

/** What cards and lists need of an akt type to show its akter. */
export const ShownType = graphql(`
	fragment ShownType on AktType {
		key
		version
		name
		fields {
			key
			name
		}
		presentation {
			card
			list
		}
	}
`);

export const TypeShape = graphql(`
	fragment TypeShape on AktType {
		key
		version
		name
		document
		fields {
			key
			name
			kind
			description
			required
			multiline
			options {
				key
				name
			}
			format
		}
		states {
			key
			name
			initial
			final
		}
		transitions {
			key
			name
			from
			to
			requires
		}
	}
`);

export const AktPage = graphql(`
	query AktPage($number: String!) {
		viewer {
			subject
		}
		aktTypes {
			key
			version
			name
		}
		akt(number: $number) {
			id
			number
			title
			revision
			state {
				key
				name
				final
				tone
			}
			type {
				...TypeShape
			}
			fields {
				...FieldValues
			}
			containers {
				id
				number
				title
				type {
					key
					version
					name
				}
			}
			board(first: 500) {
				columns {
					key
					name
					akter {
						id
						number
						title
						state {
							key
							name
							tone
						}
						type {
							...ShownType
						}
						fields {
							...FieldValues
						}
					}
				}
			}
			revisions {
				revision
				transition
				reason
				changes
				membership
				createdAt
				by {
					id
					name
				}
			}
		}
	}
`);

export const NewAkt = graphql(`
	query NewAkt($type: String!) {
		viewer {
			subject
		}
		aktType(key: $type) {
			...TypeShape
		}
	}
`);

export const Contacts = graphql(`
	query Contacts {
		contacts {
			id
			displayName
			name
			email
			user {
				id
				name
			}
		}
	}
`);

export const CreateAkt = graphql(`
	mutation CreateAkt($input: CreateAktInput!) {
		createAkt(input: $input) {
			id
			number
		}
	}
`);

export const EditAkt = graphql(`
	mutation EditAkt($input: EditAktInput!) {
		editAkt(input: $input) {
			id
			revision
		}
	}
`);

export const PerformTransition = graphql(`
	mutation PerformTransition($input: PerformTransitionInput!) {
		performTransition(input: $input) {
			id
			revision
			state {
				key
				name
			}
		}
	}
`);

export const DraftField = graphql(`
	mutation DraftField($input: DraftFieldInput!) {
		draftField(input: $input) {
			id
			revision
		}
	}
`);

export const AddToContainer = graphql(`
	mutation AddToContainer($input: MembershipInput!) {
		addToContainer(input: $input) {
			id
		}
	}
`);

export const RemoveFromContainer = graphql(`
	mutation RemoveFromContainer($input: MembershipInput!) {
		removeFromContainer(input: $input) {
			id
		}
	}
`);

export const TransferMember = graphql(`
	mutation TransferMember($input: TransferInput!) {
		transferMember(input: $input) {
			id
		}
	}
`);

export const MoveAkt = graphql(`
	mutation MoveAkt($input: MoveAktInput!) {
		moveAkt(input: $input) {
			id
			rank
		}
	}
`);

export const AddContact = graphql(`
	mutation AddContact($input: AddContactInput!) {
		addContact(input: $input) {
			id
			displayName
		}
	}
`);

export const EditContact = graphql(`
	mutation EditContact($input: EditContactInput!) {
		editContact(input: $input) {
			id
			displayName
		}
	}
`);

export const EraseContact = graphql(`
	mutation EraseContact($id: ID!) {
		eraseContact(id: $id) {
			id
			displayName
		}
	}
`);

export const Shell = graphql(`
	query Shell {
		viewer {
			subject
			user {
				id
				name
			}
		}
		aktTypes {
			key
			version
			name
			akterCount
		}
	}
`);

export const SearchAkter = graphql(`
	query SearchAkter($text: String!) {
		akter(search: $text, first: 12) {
			id
			number
			title
			state {
				key
				name
				tone
			}
			type {
				key
				version
				name
			}
		}
	}
`);
