import { Loading, Skeleton } from '../shared/Skeleton'

/** A box of the chart while it loads. */
function Node() {
  return (
    <div className="oc-node">
      <Skeleton kind="short" />
      <Skeleton />
    </div>
  )
}

/** The org chart while it loads: the title, and a small tree of boxes with its connecting lines, which a phone draws as a list. */
export function OrgChartSkeleton() {
  return (
    <Loading>
      <div className="page-head">
        <div className="page-title">
          <Skeleton kind="title" />
        </div>
      </div>
      <div className="oc-frame">
        <div className="orgchart">
          <ul>
            <li>
              <Node />
              <ul>
                {[0, 1].map((department) => (
                  <li key={department}>
                    <Node />
                    <ul>
                      {[0, 1].map((team) => (
                        <li key={team}>
                          <Node />
                        </li>
                      ))}
                    </ul>
                  </li>
                ))}
              </ul>
            </li>
          </ul>
        </div>
      </div>
    </Loading>
  )
}
