import type { RequestOption } from '../api/teamview'
import { useI18n } from '../i18n'

interface ChoiceCardsProps {
  /** Unique per item, so each item gets its own radio group. */
  name: string
  options: readonly RequestOption[]
  choice: string | null
  onChoose: (key: string) => void
  isDisabled: boolean
}

export function ChoiceCards({ name, options, choice, onChoose, isDisabled }: ChoiceCardsProps) {
  const { t } = useI18n()
  if (options.length === 0) return null
  return (
    <div className="choices" role="radiogroup" aria-label={t.desk.paper.options}>
      {options.map((option, index) => (
        <label key={option.key} className="choice">
          <input
            className="sr-only"
            type="radio"
            name={name}
            value={option.key}
            checked={choice === option.key}
            disabled={isDisabled}
            onChange={() => onChoose(option.key)}
          />
          <span className="choice-top">
            <span className="choice-key" aria-hidden="true">{index + 1}</span>
            {option.isRecommended ? <span className="choice-rec">{t.desk.paper.recommended}</span> : null}
          </span>
          <span className="choice-name">{option.text}</span>
          {option.detail === undefined ? null : <span className="choice-detail">{option.detail}</span>}
        </label>
      ))}
    </div>
  )
}
