import React from 'react';
import styles from './styles.module.css';

const RightIcon = () => (
  <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14"
    fill="currentColor" viewBox="0 0 24 24">
    <path d="M9 15.59 4.71 11.3 3.3 12.71l5 5c.2.2.45.29.71.29s.51-.1.71-.29l11-11-1.41-1.41L9.02 15.59Z" />
  </svg>
);

const WrongIcon = () => (
  <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14"
    fill="currentColor" viewBox="0 0 24 24">
    <path d="m7.76 14.83-2.83 2.83 1.41 1.41 2.83-2.83 2.12-2.12.71-.71.71.71 1.41 1.42 3.54 3.53 1.41-1.41-3.53-3.54-1.42-1.41-.71-.71 5.66-5.66-1.41-1.41L12 10.59 6.34 4.93 4.93 6.34 10.59 12l-.71.71z" />
  </svg>
);

export default function CompSec3() {
  return (
    <section className={styles.section}>
      <div className={styles.inner}>

        <div className={styles.titleRow}>
          <span className={styles.eyebrow}>Why Pallet-XP</span>
          <h2 className={styles.h1}>
            A better way to build <em className={styles.em}>reputation.</em>
          </h2>
          <div className={styles.titleRule} />
        </div>

        <div className={styles.grid}>

          <div className={`${styles.vsHalf} ${styles.vsHalfBad}`}>
            <div className={styles.imgWrapBad}>
              <img src={require('@site/static/img/CompSec-3-1.png').default} alt="Token model" className={styles.cardImg} />
            </div>
            <div className={styles.cb}>
              <p className={styles.eyBad}>Without Pallet-XP</p>
              <p className={styles.ctBad}>Tokens can be bought.</p>
              <div className={styles.hrBad} />
              <div className={styles.tag}><span className={styles.tagIconBad}><WrongIcon /></span>Inflation erodes value</div>
              <div className={styles.tag}><span className={styles.tagIconBad}><WrongIcon /></span>Supply manipulation</div>
              <div className={styles.tag}><span className={styles.tagIconBad}><WrongIcon /></span>Temporary &amp; gameable</div>
            </div>
          </div>

          <div className={`${styles.vsHalf} ${styles.vsHalfGood}`}>
            <div className={styles.imgWrapGood}>
              <img src={require('@site/static/img/CompSec-3-2.png').default} alt="Pallet-XP mascot" className={styles.cardImg} />
            </div>
            <div className={styles.cb}>
              <p className={styles.eyGood}>With Pallet-XP</p>
              <p className={styles.ctGood}>Reputation is earned.</p>
              <div className={styles.hr} />
              <div className={styles.tag}><span className={styles.tagIconGood}><RightIcon /></span>No shortcuts, ever</div>
              <div className={styles.tag}><span className={styles.tagIconGood}><RightIcon /></span>No inflation, no BS</div>
              <div className={styles.tag}><span className={styles.tagIconGood}><RightIcon /></span>Earned, not purchased</div>
            </div>
          </div>

          <div className={styles.cardFeature}>
            <div className={styles.imgWrapFeature}>
              <img src={require('@site/static/img/CompSec-3-3.png').default} alt="XP rewards real work" className={styles.cardImg} />
            </div>
            <div className={styles.cb}>
              <p className={styles.eyG}>Real work</p>
              <p className={styles.ctW}>XP rewards impact.</p>
              <div className={styles.hr} />
              <div className={styles.tag}><span className={`${styles.dot} ${styles.dotGreen}`} />Aligned incentives</div>
              <div className={styles.tag}><span className={`${styles.dot} ${styles.dotGreen}`} />Long-term trust</div>
              <div className={styles.tag}><span className={`${styles.dot} ${styles.dotGreen}`} />Reputation over noise</div>
            </div>
          </div>

        </div>

        <div className={styles.tabletGrid}>

          <div className={styles.tabletVsRow}>
            <div className={`${styles.vsHalf} ${styles.vsHalfBad}`}>
              <div className={styles.imgWrapBad}>
                <img src={require('@site/static/img/CompSec-3-1.png').default} alt="Token model" className={styles.cardImg} />
              </div>
              <div className={styles.cb}>
                <p className={styles.eyBad}>Without Pallet-XP</p>
                <p className={styles.ctBad}>Tokens can be bought.</p>
                <div className={styles.hrBad} />
                <div className={styles.tag}><span className={styles.tagIconBad}><WrongIcon /></span>Inflation erodes value</div>
                <div className={styles.tag}><span className={styles.tagIconBad}><WrongIcon /></span>Supply manipulation</div>
                <div className={styles.tag}><span className={styles.tagIconBad}><WrongIcon /></span>Temporary &amp; gameable</div>
              </div>
            </div>

            <div className={`${styles.vsHalf} ${styles.vsHalfGood}`}>
              <div className={styles.imgWrapGood}>
                <img src={require('@site/static/img/CompSec-3-2.png').default} alt="Pallet-XP mascot" className={styles.cardImg} />
              </div>
              <div className={styles.cb}>
                <p className={styles.eyGood}>With Pallet-XP</p>
                <p className={styles.ctGood}>Reputation is earned.</p>
                <div className={styles.hr} />
                <div className={styles.tag}><span className={styles.tagIconGood}><RightIcon /></span>No shortcuts, ever</div>
                <div className={styles.tag}><span className={styles.tagIconGood}><RightIcon /></span>No inflation, no BS</div>
                <div className={styles.tag}><span className={styles.tagIconGood}><RightIcon /></span>Earned, not purchased</div>
              </div>
            </div>
          </div>

          <div className={`${styles.cardFeature} ${styles.cardFeatureWide}`}>
            <div className={styles.imgWrapFeatureWide}>
              <img src={require('@site/static/img/CompSec-3-3.png').default} alt="XP rewards real work" className={styles.cardImg} />
            </div>
            <div className={styles.cb}>
              <p className={styles.eyG}>Real work</p>
              <p className={styles.ctW}>XP rewards impact.</p>
              <div className={styles.hr} />
              <div className={styles.tag}><span className={`${styles.dot} ${styles.dotGreen}`} />Aligned incentives</div>
              <div className={styles.tag}><span className={`${styles.dot} ${styles.dotGreen}`} />Long-term trust</div>
              <div className={styles.tag}><span className={`${styles.dot} ${styles.dotGreen}`} />Reputation over noise</div>
            </div>
          </div>

        </div>

        <div className={styles.mobileGrid}>

          <div className={`${styles.vsHalf} ${styles.vsHalfBad}`}>
            <div className={styles.imgWrapBad}>
              <img src={require('@site/static/img/CompSec-3-1.png').default} alt="Token model" className={styles.cardImg} />
            </div>
            <div className={styles.cb}>
              <p className={styles.eyBad}>Without Pallet-XP</p>
              <p className={styles.ctBad}>Tokens can be bought.</p>
              <div className={styles.hrBad} />
              <div className={styles.tag}><span className={styles.tagIconBad}><WrongIcon /></span>Inflation erodes value</div>
              <div className={styles.tag}><span className={styles.tagIconBad}><WrongIcon /></span>Supply manipulation</div>
              <div className={styles.tag}><span className={styles.tagIconBad}><WrongIcon /></span>Temporary &amp; gameable</div>
            </div>
          </div>

          <div className={`${styles.vsHalf} ${styles.vsHalfGood}`}>
            <div className={styles.imgWrapGood}>
              <img src={require('@site/static/img/CompSec-3-2.png').default} alt="Pallet-XP mascot" className={styles.cardImg} />
            </div>
            <div className={styles.cb}>
              <p className={styles.eyGood}>With Pallet-XP</p>
              <p className={styles.ctGood}>Reputation is earned.</p>
              <div className={styles.hr} />
              <div className={styles.tag}><span className={styles.tagIconGood}><RightIcon /></span>No shortcuts, ever</div>
              <div className={styles.tag}><span className={styles.tagIconGood}><RightIcon /></span>No inflation, no BS</div>
              <div className={styles.tag}><span className={styles.tagIconGood}><RightIcon /></span>Earned, not purchased</div>
            </div>
          </div>

          <div className={styles.cardFeature}>
            <div className={styles.imgWrapFeature}>
              <img src={require('@site/static/img/CompSec-3-3.png').default} alt="XP rewards real work" className={styles.cardImg} />
            </div>
            <div className={styles.cb}>
              <p className={styles.eyG}>Real work</p>
              <p className={styles.ctW}>XP rewards impact.</p>
              <div className={styles.hr} />
              <div className={styles.tag}><span className={`${styles.dot} ${styles.dotGreen}`} />Aligned incentives</div>
              <div className={styles.tag}><span className={`${styles.dot} ${styles.dotGreen}`} />Long-term trust</div>
              <div className={styles.tag}><span className={`${styles.dot} ${styles.dotGreen}`} />Reputation over noise</div>
            </div>
          </div>

        </div>

      </div>
    </section>
  );
}