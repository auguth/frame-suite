import React from 'react';
import styles from './styles.module.css';

const ucImages = {
  dao:          require('@site/static/img/dao.png').default,
  contribution: require('@site/static/img/contribution.png').default,
  gamified:     require('@site/static/img/gamified.png').default,
  governance:   require('@site/static/img/governance.png').default,
  identity:     require('@site/static/img/identity.png').default,
  // crosschain:   require('@site/static/img/gamified.png').default,
  // sybil:        require('@site/static/img/gamified.png').default,
  // credit:       require('@site/static/img/gamified.png').default,
};

const mascotImg = require('@site/static/img/CompSec-7-1.png').default;

const useCases = [
  { id: 'dao',          label: 'DAO Reputation'        },
  { id: 'contribution', label: 'Contribution Tracking'  },
  { id: 'gamified',     label: 'Gamified Protocols'     },
  { id: 'governance',   label: 'Governance Weighting'   },
  { id: 'identity',     label: 'Identity Scoring'       },
  // { id: 'crosschain',   label: 'Cross-chain Identity'   },
  // { id: 'sybil',        label: 'Sybil Resistance'       },
  // { id: 'credit',       label: 'On-chain Credit'        },
];

export default function CompSec7() {
  return (
    <section className={styles.section}>
      <div className={styles.inner}>

        <div className={styles.contentCol}>
          <h2 className={styles.title}>
            Endless Use Cases.<br />
            <span className={styles.titleAccent}>One Primitive.</span>
          </h2>
          <p className={styles.sub}>
            One reputation primitive. Infinitely composable. Build anything
            that needs trust - from DAOs to games to identity systems.
          </p>

          <div className={styles.tagGrid}>
            {useCases.map((uc) => (
              <div key={uc.id} className={styles.tag}>
                <div className={styles.tagIcon}>
                  <img src={ucImages[uc.id]} alt={uc.label} className={styles.tagImg} />
                </div>
                <span className={styles.tagLabel}>{uc.label}</span>
              </div>
            ))}
            <div className={`${styles.tag} ${styles.tagMore}`}>
              <span className={styles.tagMoreLabel}>...and many more</span>
            </div>
          </div>
        </div>

        <div className={styles.layout}>
          <div className={styles.mascotWrapper}>
            <img
              src={mascotImg}
              alt="Build cool stuff"
              className={styles.mascotImg}
            />
          </div>
        </div>

      </div>
    </section>
  );
}