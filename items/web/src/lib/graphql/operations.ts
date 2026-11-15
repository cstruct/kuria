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
			...TypeMark
			akterCount
			fields {
				key
				kind
			}
		}
		akter(first: 10) {
			nodes {
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
					...TypeMark
				}
			}
		}
	}
`);

export const AkterOfType = graphql(`
	query AkterOfType(
		$type: String!
		$state: String
		$after: String
		$where: [FieldCondition!]
		$outcome: Outcome
	) {
		aktType(key: $type) {
			...ShownType
			states {
				key
				name
			}
		}
		akter(type: $type, state: $state, first: 50, after: $after, where: $where, outcome: $outcome) {
			nodes {
				id
				number
				title
				rank
				updatedAt
				outcome
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
			pageInfo {
				endCursor
				hasNextPage
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
		... on UsersValue {
			field
			users {
				...SomeoneShown
			}
		}
		... on SectionsValue {
			field
			value
		}
		... on MarkdownValue {
			field
			markdown
			headings {
				id
				level
				text
			}
			references {
				destination
				broken
				pinnedVersion
			}
			images {
				destination
				broken
				attachment {
					url
					mediaType
				}
			}
			diagrams {
				source
				attachment {
					url
				}
			}
		}
		... on VerdictValue {
			field
			verdicts {
				verdict
				assignee {
					...SomeoneShown
				}
			}
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

/** How an akt type is marked wherever it is named: its icon and colour. */
export const TypeMark = graphql(`
	fragment TypeMark on AktType {
		key
		version
		name
		presentation {
			icon
			colour
		}
	}
`);

/** What cards and lists need of an akt type to show its akter. */
export const ShownType = graphql(`
	fragment ShownType on AktType {
		...TypeMark
		fields {
			key
			name
			kind
			assignees
		}
		presentation {
			card
			list
		}
		transitions {
			key
			name
			from
			to
			requires
			outcomes
		}
	}
`);

export const TypeShape = graphql(`
	fragment TypeShape on AktType {
		...TypeMark
		document
		liveDrafts
		fields {
			key
			name
			kind
			description
			required
			multiline
			multiple
			of
			assignees
			references
			diagrams
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
			outcomes
		}
	}
`);

export const AktPage = graphql(`
	query AktPage($number: String!, $version: Int) {
		viewer {
			subject
		}
		aktTypes {
			key
			version
			name
		}
		akt(number: $number, version: $version) {
			id
			number
			title
			revision
			outcome
			version {
				...VersionShown
			}
			versions {
				...VersionShown
			}
			copiedFrom {
				id
				number
				title
			}
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
						revision
						outcome
						state {
							key
							name
							tone
							final
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
			links {
				kind
				name
				outgoing
				byReference
				pinnedRevision
				pinned {
					revision
					title
					state {
						key
						name
						tone
					}
				}
				akt {
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
				named {
					...SomeoneShown
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
			linkedBy {
				id
				name
			}
		}
	}
`);

/** What a type's list can be filtered by: its fields, and who is asking. */
export const ListFilters = graphql(`
	query ListFilters($type: String!) {
		viewer {
			subject
		}
		aktType(key: $type) {
			key
			version
			fields {
				key
				name
				kind
				multiline
				multiple
				options {
					key
					name
				}
			}
		}
	}
`);

/** Akter of one type meeting conditions on their fields, such as those
 * naming the viewer. */
export const AkterWhere = graphql(`
	query AkterWhere($type: String!, $where: [FieldCondition!], $first: Int) {
		akter(type: $type, where: $where, first: $first) {
			nodes {
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
					...TypeMark
				}
			}
		}
	}
`);

/** The users who signed in to the tenant; refused to those who may not
 * read contacts. */
export const Users = graphql(`
	query Users {
		users {
			id
			name
		}
	}
`);

export const LinkContact = graphql(`
	mutation LinkContact($input: LinkContactInput!) {
		linkContact(input: $input) {
			id
			displayName
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

export const CopyAkt = graphql(`
	mutation CopyAkt($input: CopyAktInput!) {
		copyAkt(input: $input) {
			id
			number
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

export const AddLink = graphql(`
	mutation AddLink($input: LinkInput!) {
		addLink(input: $input) {
			id
			revision
		}
	}
`);

export const RemoveLink = graphql(`
	mutation RemoveLink($input: UnlinkInput!) {
		removeLink(input: $input) {
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
			...TypeMark
			akterCount
			presentation {
				navigation
			}
		}
	}
`);

export const SearchAkter = graphql(`
	query SearchAkter($text: String!) {
		akter(search: $text, first: 12) {
			nodes {
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
	}
`);

export const VersionShown = graphql(`
	fragment VersionShown on Version {
		number
		revision
		label
		releasedAt
		releasedBy {
			id
			name
		}
	}
`);

export const ReleaseVersion = graphql(`
	mutation ReleaseVersion($input: ReleaseVersionInput!) {
		releaseVersion(input: $input) {
			...VersionShown
		}
	}
`);

/** An akt's field values at one of its versions, or now, to compare. */
export const AktFieldsAt = graphql(`
	query AktFieldsAt($number: String!, $version: Int) {
		akt(number: $number, version: $version) {
			id
			version {
				number
			}
			fields {
				...FieldValues
			}
		}
	}
`);

/** What a reference to an akt can name: its headings and versions. */
export const AktReferable = graphql(`
	query AktReferable($number: String!) {
		akt(number: $number) {
			id
			number
			title
			versions {
				number
				label
			}
			fields {
				... on MarkdownValue {
					field
					headings {
						id
						level
						text
					}
				}
			}
		}
	}
`);

export const ImageTypes = graphql(`
	query ImageTypes {
		aktTypes {
			key
			name
			presentation {
				title
				image
			}
		}
	}
`);

export const ImagePicture = graphql(`
	query ImagePicture($number: String!) {
		akt(number: $number) {
			number
			title
			type {
				presentation {
					image
				}
			}
			fields {
				... on AttachmentValue {
					field
					attachment {
						url
					}
				}
			}
		}
	}
`);

export const RenderDiagram = graphql(`
	query RenderDiagram($source: String!) {
		renderDiagram(source: $source) {
			svg
			error
		}
	}
`);
