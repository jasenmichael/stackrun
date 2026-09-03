import BrowserOnly from "@docusaurus/BrowserOnly";
import useBaseUrl from "@docusaurus/useBaseUrl";
import { useEffect, useRef } from "react";
import "asciinema-player/dist/bundle/asciinema-player.css";

function Player() {
  const ref = useRef(null);
  const cast = useBaseUrl("/demo.cast");

  useEffect(() => {
    let instance;
    import("asciinema-player").then((mod) => {
      if (ref.current) {
        instance = mod.create(cast, ref.current, {
          preload: true,
          fit: "width",
          loop: true,
        });
      }
    });
    return () => {
      instance?.dispose?.();
    };
  }, [cast]);

  return <div ref={ref} className="cast" id="demo" />;
}

export default function AsciinemaDemo() {
  return (
    <BrowserOnly fallback={<div id="demo" className="cast" />}>
      {() => <Player />}
    </BrowserOnly>
  );
}
