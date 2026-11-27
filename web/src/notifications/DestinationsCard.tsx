import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { useApi } from '../api/context'
import { aboutQuery, DESTINATIONS_KEY, devicesQuery, pushSubscriptionsQuery, pushTokensQuery } from '../api/queries'
import { useI18n } from '../i18n'
import { useToast } from '../shared/Toasts'

/** One place the node sends notices to: a browser's subscription or an app's token, as the node lists it. */
interface Destination {
  kind: 'browser' | 'app'
  id: string
  label: string
  registeredAt: string
  /** The device it names; none for a registration that names none. */
  device: string | undefined
}

const messageOf = (failure: unknown): string => (failure instanceof Error ? failure.message : String(failure))

/**
 * Every browser and app the node sends notices to, with when it registered and which device it is tied to, and a confirmed way to stop
 * notices to one. A node that ties registrations to devices drops them when a device is removed; this is the way out for the rest,
 * the old ones that name no device, and for any that should not get notices.
 */
export function DestinationsCard() {
  const { t, fmt } = useI18n()
  const text = t.notices.destinations
  const api = useApi()
  const queryClient = useQueryClient()
  const toast = useToast()
  const browsers = useQuery(pushSubscriptionsQuery(api))
  const apps = useQuery(pushTokensQuery(api))
  const devices = useQuery(devicesQuery(api))
  const about = useQuery(aboutQuery(api))
  const [asking, setAsking] = useState<string | undefined>()
  const stop = useMutation({
    mutationFn: (destination: Destination) => (destination.kind === 'browser' ? api.removePushSubscription(destination.id) : api.removePushToken(destination.id)),
    onSuccess: (_done, destination) => {
      toast(text.stopped(destination.label))
      setAsking(undefined)
    },
    onError: (failure) => toast(text.stopFailed(messageOf(failure)), 'error'),
    onSettled: () => queryClient.invalidateQueries({ queryKey: DESTINATIONS_KEY }),
  })
  const failure = browsers.error ?? apps.error
  if (browsers.data === undefined || apps.data === undefined) {
    return (
      <section className="device-card" aria-labelledby="destinations">
        <h2 className="sub-head" id="destinations">{text.title}</h2>
        {failure === null ? <p className="muted">{text.loading}</p> : <p className="empty" role="alert">{text.failed(messageOf(failure))}</p>}
      </section>
    )
  }
  // The device a registration names can be said only by a node that ties them to devices.
  const ties = about.data?.capabilities.includes('pushDevice') === true
  const deviceLabels = new Map((devices.data?.orgs ?? [] /* stand-in-ok(optional): before the devices are read a device is named by its id, not by a label that is not known */).flatMap((org) => org.devices.map((device) => [device.id, device.label] as const)))
  const destinations: Destination[] = [
    ...browsers.data.subscriptions.map((one): Destination => ({ kind: 'browser', id: one.id, label: one.label, registeredAt: one.registeredAt, device: one.device })),
    ...apps.data.tokens.map((one): Destination => ({ kind: 'app', id: one.id, label: one.label, registeredAt: one.registeredAt, device: one.device })),
  ].sort((first, second) => Date.parse(second.registeredAt) - Date.parse(first.registeredAt))
  const tiedLine = (destination: Destination): string | null => {
    if (!ties) return null
    if (destination.device === undefined) return text.tiedToNone
    const label = deviceLabels.get(destination.device)
    return label === undefined && devices.data !== undefined ? text.tiedToGone(destination.device) : text.tiedTo(label ?? destination.device)
  }
  return (
    <section className="device-card" aria-labelledby="destinations">
      <h2 className="sub-head" id="destinations">{text.title}</h2>
      <p className="muted">{text.lead}</p>
      {destinations.length === 0 ? <p className="muted">{text.none}</p> : null}
      <ul className="destination-list" aria-labelledby="destinations">
        {destinations.map((destination) => {
          const key = `${destination.kind}:${destination.id}`
          const when = fmt.relative(destination.registeredAt, t.time.justNow)
          const tied = tiedLine(destination)
          return (
            <li key={key}>
              <p>
                <b>{destination.label}</b> <span className="muted">{text.kinds[destination.kind]} · {text.registered(when)}</span>
              </p>
              {tied === null ? null : <p className="muted">{tied}</p>}
              {asking === key ? (
                <div className="device-remove-panel" role="group" aria-label={text.confirmTitle(destination.label)}>
                  <p><b>{text.confirmTitle(destination.label)}</b></p>
                  <p>{text.confirmBody(destination.label, when)}</p>
                  <div className="device-remove-actions">
                    <button className="btn primary" type="button" disabled={stop.isPending} onClick={() => stop.mutate(destination)}>
                      {stop.isPending ? text.stopping : text.confirm}
                    </button>
                    <button className="btn" type="button" disabled={stop.isPending} onClick={() => setAsking(undefined)}>
                      {text.cancel}
                    </button>
                  </div>
                </div>
              ) : (
                <button className="btn small" type="button" aria-label={text.stopFor(destination.label)} onClick={() => setAsking(key)}>
                  {text.stop}
                </button>
              )}
            </li>
          )
        })}
      </ul>
    </section>
  )
}
