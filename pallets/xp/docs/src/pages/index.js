import React from 'react';
import Layout from '@theme/Layout';
import HomeNavbar from '@site/src/components/HomeNavbar';
import HeroSection from '@site/src/components/HeroSection';
import CompSec2 from '@site/src/components/CompSec2';
import CompSec3 from '@site/src/components/CompSec3';
import CompSec4 from '@site/src/components/CompSec4';
import CompSec5 from '@site/src/components/CompSec5';
import CompSec6 from '@site/src/components/CompSec6';
import CompSec7 from '@site/src/components/CompSec7';
import CompSec8 from '@site/src/components/CompSec8';
import CompSec9 from '@site/src/components/CompSec9';
import HomeFooter from '@site/src/components/HomeFooter';
import styles from './index.module.css';

export default function Home() {
  return (
    <Layout title="Pallet XP" description="A reputation primitive for Web3." noFooter hideNavbar>
      <div className={styles.page}>
        {/* <HomeNavbar /> */}
        <main>
          <HeroSection />
          <CompSec2 />
          <CompSec3 />
          <CompSec4 />
          <CompSec5 />
          <CompSec6 />
          <CompSec7 />
          <CompSec8 />
          <CompSec9 />
          <HomeFooter/>
        </main>
      </div>
    </Layout>
  );
}