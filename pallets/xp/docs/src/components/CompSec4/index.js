import React from 'react';
import styles from './styles.module.css';

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

const STEPS = [
  {
    num: '01',
    icon: '🌱',
    title: 'Start',
    desc: <>Register an <Kw>XpId</Kw> and establish a reputation identity. Every XP key acts as a dedicated participation context - owned by an account and ready to grow through activity.</>,
    fns: ['begin_xp(owner, key, points)'],
  },
  {
    num: '02',
    icon: '⚡',
    title: 'Earn XP',
    desc: <>Actions first build reputation (<Kw>Pulse</Kw>), then unlock scaled XP growth. Consistent participation earns more over time, while burst activity and spam provide diminishing advantage.</>,
    fns: ['earn_xp(key, points)'],
  },
  {
    num: '03',
    icon: '🔒',
    title: 'Use Constraints',
    desc: 'Reserve or lock XP to represent intent and commitment. Use reputation as a programmable primitive for staking, governance, access control, and protocol-level mechanics.',
    fns: ['lock_xp(key, reason, points)', 'reserve_xp(key, reason, points)'],
  },
  {
    num: '04',
    icon: '👻',
    title: 'Inactive? Reaped.',
    desc: 'Inactive XP identities can be cleaned up by the runtime once they fall below activity requirements. Reputation stays meaningful by rewarding active participation instead of passive accumulation.',
    fns: ['reap_xp(key)'],
  },
];

export default function CompSec4() {
  return (
    <section className={styles.section}>
      <div className={styles.inner}>

        <div className={styles.titleRow}>
          <span className={styles.eyebrow}>How It Works</span>
          <h2 className={styles.h1}>
            Simple by Design.
            <em className={styles.em}> Powerful</em> by Default.
          </h2>
          <div className={styles.titleRule} />
        </div>

        <div className={styles.bodyGrid}>

          <div className={styles.lifecycle}>
            {STEPS.map((step, i) => (
              <React.Fragment key={step.num}>
                <div className={`${styles.fStep} ${i === 0 ? styles.fStepFirst : ''}`}>
                  <div className={styles.fNum}>{step.num}</div>
                  <div className={styles.fAccentVr} />
                  <div className={styles.fBody}>
                    <div className={styles.fIconRow}>
                      <span className={styles.fIcon}>{step.icon}</span>
                      <span className={styles.fName}>{step.title}</span>
                    </div>
                    <p className={styles.fDesc}>{step.desc}</p>
                    <div className={styles.fnRow}>
                      {step.fns.map((fn) => (
                        <span key={fn} className={styles.fn}>{fn}</span>
                      ))}
                    </div>
                  </div>
                </div>
                {i < STEPS.length - 1 && (
                  <div className={styles.stepDivider} />
                )}
              </React.Fragment>
            ))}
          </div>

          <div className={styles.cardsCol}>

            <div className={styles.legCard}>
              <img
                src={require('@site/static/img/CompSec-4-1.png').default}
                alt="State legend"
                className={styles.cardFullImg}
              />
            </div>

            <div className={styles.mascotCard}>
              <img
                src={require('@site/static/img/CompSec-4-2.png').default}
                alt="Pallet XP mascot"
                className={styles.cardFullImg}
              />
            </div>

          </div>

        </div>
      </div>
    </section>
  );
}