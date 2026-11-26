// The ids of the decisions this page sent, in this session: a success this client caused is not news to it.
const sent = new Set<string>()

export const rememberSent = (id: string): void => void sent.add(id)

export const sentDecisionIds = (): string[] => [...sent]
