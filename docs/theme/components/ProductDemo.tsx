import { useState } from 'react';
import {
  demoFixtures,
  stateLabels,
  type DemoModule,
  type DemoState,
} from '../fixtures/productDemo';
import './ProductDemo.css';

type Props = { module: DemoModule; state?: DemoState };

export function ProductDemo({ module, state: initialState = 'normal' }: Props) {
  const [state, setState] = useState(initialState);
  const fixture = demoFixtures[module][state];

  return (
    <section
      className={`product-demo product-demo--${state}`}
      aria-label={`${fixture.title}产品演示`}
    >
      <header className="product-demo__header">
        <div>
          <span className="product-demo__eyebrow">{fixture.eyebrow}</span>
          <h2>{fixture.title}</h2>
        </div>
        <div
          className="product-demo__states"
          role="group"
          aria-label="演示状态"
        >
          {(Object.keys(stateLabels) as DemoState[]).map((key) => (
            <button
              className={key === state ? 'is-active' : ''}
              key={key}
              type="button"
              onClick={() => setState(key)}
              aria-pressed={key === state}
            >
              {stateLabels[key]}
            </button>
          ))}
        </div>
      </header>
      <p className="product-demo__summary">{fixture.summary}</p>
      <div className="product-demo__metrics">
        {fixture.metrics.map((metric) => (
          <div className="product-demo__metric" key={metric.label}>
            <span>{metric.label}</span>
            <strong>{metric.value}</strong>
          </div>
        ))}
      </div>
      {state === 'loading' ? (
        <div className="product-demo__loading" role="status">
          <span />
          正在加载演示数据
        </div>
      ) : state === 'error' ? (
        <div className="product-demo__message" role="alert">
          无法加载演示数据
        </div>
      ) : state === 'empty' ? (
        <div className="product-demo__message">暂无数据</div>
      ) : (
        <div className="product-demo__rows">
          {fixture.rows.map((row) => (
            <div
              className="product-demo__row"
              key={`${row.name}-${row.detail}`}
            >
              <span className="product-demo__dot" />
              <div>
                <strong>{row.name}</strong>
                <small>{row.detail}</small>
              </div>
              <b>{row.status}</b>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}
