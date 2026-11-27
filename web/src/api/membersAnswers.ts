// The members answer comes from the node and is read once, here, before a screen sees it: every list is there (the node leaves out only `held`
// and what a member has no use for), a role and a key kind are ones the screen knows how to say, and an id or a label that is not text refuses the
// whole answer, so that a screen never draws a person it cannot name. The node never sends an address or a key; nothing here reads one.
import type { KeyKind, MemberDevice, OrgMember, OrgMembers, OrgParty, RootChangeRef } from './clientApi'
import { MalformedAnswer } from './evaluationAnswers'

type Raw = Record<string, unknown>

const asObject = (value: unknown): Raw => (typeof value === 'object' && value !== null && !Array.isArray(value) ? (value as Raw) : {})
const asList = (value: unknown, what: string): unknown[] => {
  if (!Array.isArray(value)) throw new MalformedAnswer(what)
  return value
}

function text(value: unknown, what: string): string {
  if (typeof value !== 'string' || value === '') throw new MalformedAnswer(what)
  return value
}

const optionalText = (value: unknown, what: string): { [key: string]: string } | Record<string, never> => (value === undefined ? {} : { [what]: text(value, what) })

function deviceOf(raw: unknown): MemberDevice {
  const given = asObject(raw)
  const keyKind = given.keyKind
  if (keyKind !== 'ed25519' && keyKind !== 'p256') throw new MalformedAnswer('key kind')
  return { id: text(given.id, 'device'), label: text(given.label, 'device label'), keyKind: keyKind as KeyKind, addedAt: text(given.addedAt, 'device time') }
}

function memberOf(raw: unknown): OrgMember {
  const given = asObject(raw)
  const role = given.role
  if (role !== 'member' && role !== 'answerer') throw new MalformedAnswer('role')
  if (typeof given.isHosted !== 'boolean') throw new MalformedAnswer('hosted flag')
  if (given.hasSignIn !== undefined && typeof given.hasSignIn !== 'boolean') throw new MalformedAnswer('sign-in flag')
  return {
    id: text(given.id, 'member'),
    label: text(given.label, 'member label'),
    role,
    ...(optionalText(given.department, 'department') as { department?: string }),
    isHosted: given.isHosted,
    ...(given.hasSignIn === undefined ? {} : { hasSignIn: given.hasSignIn }),
    devices: asList(given.devices, 'devices').map(deviceOf),
  }
}

function partyOf(raw: unknown): OrgParty {
  const given = asObject(raw)
  return {
    id: text(given.id, 'party'),
    label: text(given.label, 'party label'),
    ...(optionalText(given.department, 'department') as { department?: string }),
    askAbout: asList(given.askAbout, 'topics').map((topic) => text(topic, 'topic')),
    answeredBy: asList(given.answeredBy, 'answerers').map((one) => text(one, 'answerer')),
  }
}

function heldOf(raw: unknown): RootChangeRef {
  const given = asObject(raw)
  return {
    id: text(given.id, 'held change'),
    org: text(given.org, 'held change org'),
    kind: text(given.kind, 'held change kind') as RootChangeRef['kind'],
    member: text(given.member, 'held change member'),
    ...(optionalText(given.label, 'label') as { label?: string }),
    ...(optionalText(given.device, 'device') as { device?: string }),
    what: text(given.what, 'held change words'),
    receivedAt: text(given.receivedAt, 'held change time'),
  }
}

export function readMembers(raw: unknown): OrgMembers {
  const given = asObject(raw)
  return {
    org: text(given.org, 'org'),
    members: asList(given.members, 'members').map(memberOf),
    seats: asList(given.seats, 'seats').map(partyOf),
    externals: asList(given.externals, 'externals').map(partyOf),
    ...(given.held === undefined ? {} : { held: asList(given.held, 'held changes').map(heldOf) }),
  }
}
