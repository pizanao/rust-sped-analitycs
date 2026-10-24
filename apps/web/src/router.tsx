import {
  createRouter,
  createRoute,
  createRootRoute,
} from '@tanstack/react-router';

import App from './App';

// Root Route vinculada ao layout e estado global do App
const rootRoute = createRootRoute({
  component: App,
});

// Sub Routes
const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/',
});

const ingestaoRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/ingestao',
});

const fiscalRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/fiscal',
});

const grcRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/grc',
});

const indicadoresRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/indicadores',
});

const routeTree = rootRoute.addChildren([
  indexRoute,
  ingestaoRoute,
  fiscalRoute,
  grcRoute,
  indicadoresRoute,
]);

export const router = createRouter({ routeTree });

declare module '@tanstack/react-router' {
  interface Register {
    router: typeof router;
  }
}
