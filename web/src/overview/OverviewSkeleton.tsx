import { Loading, Skeleton } from '../shared/Skeleton'

const CARDS = 3

/** The overview while it loads: a line for the lead, and an org's worth of team cards. */
export function OverviewSkeleton() {
  return (
    <Loading>
      <p className="ov-lead">
        <Skeleton kind="short" />
      </p>
      <section className="ov-org">
        <h2 className="sub-head">
          <Skeleton kind="title" />
        </h2>
        <div className="ov-teams">
          {Array.from({ length: CARDS }, (_, index) => (
            <article key={index} className="ov-team">
              <Skeleton kind="short" />
              <Skeleton />
              <Skeleton />
            </article>
          ))}
        </div>
      </section>
    </Loading>
  )
}
