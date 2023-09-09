import { useState } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import reactLogo from "@app/assets/react.svg";
import viteLogo from "@app/assets/vite.svg";
import "@app/App.css";

const App = () => {
  const [isEngineInitialized, setEngineInitialized] = useState<boolean>(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const handleButtonClick = async () => {
    try {
      setErrorMessage(null);
      if (!isEngineInitialized) {
        await invoke("initialize_engine", { numNodes: 3 });
        setEngineInitialized(true);
      } else {
        await invoke("destroy_engine");
        setEngineInitialized(false);
      }
    } catch (error) {
      console.error("Failed to invoke command:", error);
      setErrorMessage(
        (error as string | null) ?? ("Command failed to execute" as string)
      );
    }
  };

  return (
    <>
      <div>
        <a href="https://vitejs.dev" target="_blank">
          <img src={viteLogo} className="logo" alt="Vite logo" />
        </a>
        <a href="https://react.dev" target="_blank">
          <img src={reactLogo} className="logo react" alt="React logo" />
        </a>
      </div>
      <h1>Vite + React</h1>
      <div className="card">
        <button onClick={handleButtonClick}>
          Engine is {isEngineInitialized ? "initialized" : "not initialized"}
        </button>
        {errorMessage && <p className="text-red-500">{errorMessage}</p>}
        <p>
          Edit <code>src/App.tsx</code> and save to test HMR
        </p>
      </div>
      <p className="read-the-docs">
        Click on the Vite and React logos to learn more
      </p>
    </>
  );
};

export default App;
