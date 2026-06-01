import React from 'react';
import styles from './styles.module.css';

const cardImages = {
  pulse:     require('@site/static/img/CompSec-6-1.png').default,
  lock:      require('@site/static/img/CompSec-6-2.png').default,
  lifecycle: require('@site/static/img/CompSec-6-3.png').default,
  transfer:  require('@site/static/img/CompSec-6-4.png').default,
};

const Dot = ({ color }) => (
  <span style={{
    display: 'inline-block',
    width: 8, height: 8,
    borderRadius: '50%',
    background: color,
    flexShrink: 0,
  }} />
);

const Kw = ({ children }) => (
  <code style={{
    fontFamily: "'Fira Code', monospace",
    fontSize: '0.82em',
    background: 'rgba(127, 95, 255, 0.1)',
    border: '1px solid rgba(127, 95, 255, 0.2)',
    borderRadius: 4,
    padding: '0.1em 0.35em',
    color: '#c4b5fd',
  }}>{children}</code>
);

const featured = {
  eyebrow: '01 - Pulse Engine',
  title: 'Pulse (Reputation Engine)',
  desc: <>Actions build <Kw>pulse</Kw> first. Only once <Kw>pulse</Kw> {'>='} <Kw>MinPulse</Kw> does XP start accumulating - scaled by reputation.</>,
  points: [
    { text: <>Actions increment <Kw>pulse</Kw>, not XP directly</>, color: '#4ade80' },
    { text: <>XP only flows once <Kw>pulse</Kw> {'>='} <Kw>MinPulse</Kw></>, color: '#4ade80' },
    { text: <><Kw>PulseFactor</Kw> controls how fast <Kw>pulse</Kw> grows</>, color: '#4ade80' },
  ],
};

const smallCards = [
  {
    id: 'lock',
    eyebrow: '02 - Constraints',
    title: 'Reserve vs Lock',
    points: [
      { text: <><Kw>Reserve</Kw> - soft hold, intent-based, withdrawable</>, color: '#7c5cfc' },
      { text: <><Kw>Lock</Kw> - strict hold, no partial release</>, color: '#fb923c' },
      { text: 'Any active lock protects the key from reaping', color: '#4ade80' },
    ],
  },
  {
    id: 'lifecycle',
    eyebrow: '03 - Lifecycle',
    title: 'Lifecycle & Reaping',
    points: [
      { text: 'Every XP key must stay active to survive',        color: '#4ade80' },
      { text: 'Inactivity long enough - the key can be reaped',  color: '#fb923c' },
      { text: 'Once reaped, that key ID is blocked forever',     color: '#818cf8' },
    ],
  },
  {
    id: 'transfer',
    eyebrow: '04 - Ownership',
    title: 'Non-transferable XP',
    points: [
      { text: 'No total supply, no inflation, no issuance tracking', color: '#fb923c' },
      { text: 'XP is bound to the key, not the account holding it', color: '#4ade80' },
      { text: 'Only key ownership can be transfered', color: '#4ade80' },
    ],
  },
];

export default function CompSec6() {
  return (
    <section className={styles.section}>
      <div className={styles.inner}>

        <div className={styles.pageTitle}>
          <h2 className={styles.sectionTitle}>
            Built-in Mechanics{' '}
            <span className={styles.titleAccent}>that Just Make Sense</span>
          </h2>
          <div className={styles.titleRule} />
        </div>

        <div className={styles.bento}>

          <div className={`${styles.card} ${styles.cardLg}`}>
            <div className={styles.cardImgLg}>
              <img src={cardImages.pulse} alt={featured.title} className={styles.cardImg} />
            </div>
            <div className={styles.cardBody}>
              <span className={styles.eyebrow}>{featured.eyebrow}</span>
              <h3 className={styles.cardTitle}>{featured.title}</h3>
              <p className={styles.cardDesc}>{featured.desc}</p>
              <ul className={styles.points}>
                {featured.points.map((p, i) => (
                  <li key={i} className={styles.point}>
                    <span className={styles.pointIcon}><Dot color={p.color} /></span>
                    <span className={styles.pointText}>{p.text}</span>
                  </li>
                ))}
              </ul>
            </div>
          </div>

          {smallCards.map((c) => (
            <div key={c.id} className={`${styles.card} ${styles.cardSm}`}>
              <div className={styles.cardImgSm}>
                <img src={cardImages[c.id]} alt={c.title} className={styles.cardImg} />
              </div>
              <div className={styles.cardBody}>
                <span className={styles.eyebrow}>{c.eyebrow}</span>
                <h3 className={styles.cardTitle}>{c.title}</h3>
                <ul className={styles.points}>
                  {c.points.map((p, i) => (
                    <li key={i} className={styles.point}>
                      <span className={styles.pointIcon}><Dot color={p.color} /></span>
                      <span className={styles.pointText}>{p.text}</span>
                    </li>
                  ))}
                </ul>
              </div>
            </div>
          ))}

          <div className={`${styles.card} ${styles.cardQuote}`}>
            <div className={styles.quoteLeft}>
              <span className={styles.eyebrow}>Design principle</span>
              <blockquote className={styles.quote}>
                "Reputation is earned,<br />not bought."
              </blockquote>
            </div>
            <div className={styles.quoteDivider} />
            <div className={styles.quoteRight}>
              <p className={styles.quoteDesc}>
                Every mechanic in Pallet-XP enforces this at the protocol level.
                No shortcuts. No workarounds. Reputation means something here.
              </p>
            </div>
          </div>

        </div>
      </div>
    </section>
  );
}