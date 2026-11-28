// Every user-visible string lives in this catalog, one module per area of the app. Other locales implement the same `Messages` type,
// so a missing or mistyped key is a compile error.
import type { Around } from './around'
import { app, nav, settings, person, time, loading } from './shell'
import { overview } from './overview'
import { claim } from './claim'
import { notices } from './notices'
import { people } from './people'
import { org } from './org'
import { costs } from './costs'
import { devices } from './devices'
import { desk } from './desk'
import { members } from './members'
import { history } from './history'
import { outcomes } from './outcomes'
import { metrics } from './metrics'
import { evaluation } from './evaluation'
import { live } from './live'
import { install } from './install'

export type { Around } from './around'

export const en = {
  app,
  nav,
  settings,
  person,
  time,
  overview,
  claim,
  notices,
  people,
  org,
  costs,
  devices,
  desk,
  members,
  history,
  outcomes,
  metrics,
  evaluation,
  loading,
  live,
  install,
} as const

type Widen<T> = T extends string
  ? string
  : T extends readonly [string, string]
    ? Around
    : T extends readonly string[]
      ? readonly string[]
      : T extends (...args: infer A) => infer R
        ? (...args: A) => Widen<R>
        : T extends object
          ? { -readonly [K in keyof T]: Widen<T[K]> }
          : T

export type Messages = Widen<typeof en>
