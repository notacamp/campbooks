import { createContext, useContext } from "react";

/** Whether the content sits in a settings card, where lists drop their own panel. */
const InCardContext = createContext(false);

export const InCardProvider = InCardContext.Provider;

export const useInCard = () => useContext(InCardContext);
