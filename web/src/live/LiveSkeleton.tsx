import { Loading, Skeleton } from '../shared/Skeleton'

/** The live page while the teams are found: the tiles, the picture, and a list. */
export function LiveSkeleton() {
  return (
    <Loading>
      <div className="lv-grid">
        <div className="lv-tiles">
          {[0, 1, 2].map((one) => (
            <div key={one} className="lv-tile">
              <Skeleton kind="short" />
              <Skeleton kind="title" />
            </div>
          ))}
        </div>
        <div className="lv-panel lv-graph-panel">
          <Skeleton kind="box" />
        </div>
        <div className="lv-panel lv-feed-panel">
          {[0, 1, 2, 3].map((one) => (
            <Skeleton key={one} />
          ))}
        </div>
      </div>
    </Loading>
  )
}
