import { Loading, Skeleton } from '../shared/Skeleton'

const MATES = 3

/** A row of participant cards while a team's participants load; quiet, since the page is already there. */
export function MateSkeletons() {
  return (
    <Loading quiet>
      <div className="people">
        {Array.from({ length: MATES }, (_, index) => (
          <div key={index} className="mate">
            <div className="mate-body" style={{ flex: 1 }}>
              <Skeleton kind="short" />
              <Skeleton />
              <Skeleton />
            </div>
          </div>
        ))}
      </div>
    </Loading>
  )
}

/** The participants and work while the viewer's teams are found: a team's heading and its participant cards. */
export function PeopleSkeleton() {
  return (
    <Loading>
      <section className="crew">
        <header className="crew-head">
          <h2>
            <Skeleton kind="title" />
          </h2>
        </header>
        <MateSkeletons />
      </section>
    </Loading>
  )
}
