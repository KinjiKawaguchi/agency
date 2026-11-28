// The Japanese catalog: the same `Messages` as the English one, one module per area.
import type { Messages } from '../en'
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

export const ja: Messages = {
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
}
