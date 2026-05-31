import React from 'react';
import { useLocation } from '@docusaurus/router';
import OriginalNavbar from '@theme-original/Navbar';
import HomeNavbar from '@site/src/components/HomeNavbar';

export default function Navbar(props) {
  const { pathname } = useLocation();

  const isHome =
    pathname === '/' ||
    pathname === '/frame-suite/pallet-xp' ||
    pathname === '/frame-suite/pallet-xp/';

  return (
    <>
      <HomeNavbar />
      <div
        style={{
          position: 'fixed',
          top: 0,
          left: 0,
          width: 0,
          height: 0,
          overflow: 'visible',
          visibility: 'hidden',
          pointerEvents: 'none',
          zIndex: 999,
        }}
      >
        <OriginalNavbar {...props} />
      </div>
    </>
  );
}