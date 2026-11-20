import type { AnchorHTMLAttributes } from 'react';
import { routeHref } from '../routes.js';
import type { AppRoute } from '../routes.js';

type Props = Omit<AnchorHTMLAttributes<HTMLAnchorElement>, 'href' | 'onClick'> & {
  route: AppRoute;
  onNavigate?: () => void;
};

// Only an ordinary same-tab click enters SPA navigation. The browser owns
// context menus, modifier clicks, middle-click and explicit target windows.
export function NavigationLink({ route, onNavigate, className = '', ...props }: Props) {
  return <a {...props} className={`navigation-link ${className}`.trim()} href={routeHref(route)} onClick={event => {
    if (!onNavigate || event.defaultPrevented || event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey || props.download !== undefined || props.target && props.target !== '_self') return;
    event.preventDefault();
    onNavigate();
  }} />;
}
