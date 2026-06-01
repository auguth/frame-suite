import React from 'react';
import styles from './styles.module.css';

const images = {
  step1: require('@site/static/img/CompSec-5-1.png').default,
  step2: require('@site/static/img/CompSec-5-2.png').default,
  step3: require('@site/static/img/CompSec-5-3.png').default,
};

const xpFields = [
  { label: 'Free XP', tag: 'spendable', color: '#4ade80' },
  { label: 'Reserved XP', tag: 'usable', color: '#7c5cfc' },
  { label: 'Lock XP', tag: 'restricted', color: '#f87171' },
  { label: 'Pulse', tag: 'reputation', color: '#fb923c' },
  { label: 'Timestamp', tag: 'activity', color: '#818cf8' },
];

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

const steps = [
  {
    id: 'step1',
    num: '01',
    head: 'You own the key.\nYou own the XP.',
    sub: <>Your <Kw>XpId</Kw> holds your reputation. As long as you hold the key, everything inside belongs to you.</>,
    badge: 'Identity-bound',
  },
  {
    id: 'step2',
    num: '02',
    head: 'Transfer ownership,\nnot the XP.',
    sub: 'Transfer the key ownership to someone else and the XP inside goes with it - not back to your account.',
    badge: 'Ownership-based transfer',
  },
  {
    id: 'step3',
    num: '03',
    head: 'Same XP.\nNew owner.',
    sub: 'The new holder gets the key as-is. The reputation record stays intact - nothing resets, nothing is lost.',
    badge: 'Reputation intact',
  },
];

const HeadText = ({ text }) =>
  text.split('\n').map((line, i, arr) => (
    <React.Fragment key={i}>
      {line}{i < arr.length - 1 && <br />}
    </React.Fragment>
  ));

export default function CompSec5() {
  return (
    <section className={styles.section}>
      <div className={styles.inner}>

        <div className={styles.pageTitle}>
          <span className={styles.titleLine}>One <span className={styles.titleAccent}>Key</span>. All Your <span className={styles.titleAccent}>XP</span> Stuff.</span>
          <div className={styles.titleRule} />
        </div>

        <div className={styles.openGrid}>

          <div className={styles.colLeft}>
            <span className={styles.eyebrow}>XpId (key)</span>
            <h3 className={styles.colHeading}>Reputation lives in keys.</h3>
            <p className={styles.subHeading}>One per context. Fully on-chain.</p>
            <ul className={styles.fieldList}>
              {xpFields.map((f, i) => (
                <li key={i} className={styles.fieldItem}>
                  <span className={styles.fieldDot} style={{ background: f.color }} />
                  <span className={styles.fieldLabel}>{f.label}</span>
                  <span className={styles.fieldTag}>{f.tag}</span>
                </li>
              ))}
            </ul>
          </div>

          <div className={styles.colRule} />

          <div className={styles.colRight}>
            <div className={styles.stepGrid}>
              {steps.map((s) => (
                <div key={s.id} className={styles.stepCard}>

                  <div className={styles.stepCardImg}>
                    <img src={images[s.id]} alt={s.head} className={styles.stepImg} />
                  </div>

                  <div className={styles.imgRule} />

                  <div className={styles.stepCardText}>
                    <span className={styles.stepNum}>{s.num}</span>
                    <p className={styles.stepHead}><HeadText text={s.head} /></p>
                    <p className={styles.stepSub}>{s.sub}</p>
                    <div className={styles.stepBadge}>
                      <span className={styles.stepBadgeDot} />
                      <span className={styles.stepBadgeText}>{s.badge}</span>
                    </div>
                  </div>

                </div>
              ))}
            </div>
          </div>

        </div>
      </div>
    </section>
  );
}