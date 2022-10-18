const rust = import('./pkg');

rust
  .then(m => {
      return m.run("rustwasm/wasm-bindgen").then(() => {
        console.log("m has run");
      })
  })
  .catch(console.error);