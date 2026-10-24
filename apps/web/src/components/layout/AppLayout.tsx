import React, { useState } from 'react';
import { Sidebar, type NavItem } from './Sidebar';
import { Header } from './Header';

interface AppLayoutProps {
  activeTab: NavItem;
  onTabChange: (tab: NavItem) => void;
  title: string;
  subtitle: string;
  children: React.ReactNode;
  copilotDrawer?: React.ReactNode;
  isCopilotOpen: boolean;
  onToggleCopilot: () => void;
  onRefresh?: () => void;
  isRefreshing?: boolean;
}

export const AppLayout: React.FC<AppLayoutProps> = ({
  activeTab,
  onTabChange,
  title,
  subtitle,
  children,
  copilotDrawer,
  isCopilotOpen,
  onToggleCopilot,
  onRefresh,
  isRefreshing,
}) => {
  const [collapsed, setCollapsed] = useState(false);

  return (
    <div className="flex h-screen w-full overflow-hidden bg-slate-50 text-slate-900 font-sans">
      {/* Sidebar Navigation */}
      <Sidebar
        activeTab={activeTab}
        onTabChange={onTabChange}
        collapsed={collapsed}
        onToggleCollapse={() => setCollapsed((prev) => !prev)}
        onToggleCopilot={onToggleCopilot}
        isCopilotOpen={isCopilotOpen}
      />

      {/* Main Content Area */}
      <div className="flex flex-1 flex-col overflow-hidden">
        <Header
          title={title}
          subtitle={subtitle}
          onOpenCopilot={onToggleCopilot}
          isCopilotOpen={isCopilotOpen}
          onRefresh={onRefresh}
          isRefreshing={isRefreshing}
        />

        <main className="flex-1 overflow-y-auto p-6 md:p-8">
          <div className="mx-auto max-w-7xl">{children}</div>
        </main>
      </div>

      {/* Copilot Drawer (Slide-over) */}
      {copilotDrawer}
    </div>
  );
};
