// Agentless Monitor - Frontend JavaScript
class MonitorApp {
    constructor() {
        this.servers = [];
        this.currentServer = null;
        this.monitoringInterval = null;
        this.isMonitoring = false;
        this.serverConnections = new Map(); // Track connection status
        this.serverRetryCount = new Map(); // Track retry attempts
        this.serverTimers = new Map(); // Individual server timers
        this.serverQueues = new Map(); // Request queues for each server
        this.pendingPasswordServer = null; // Server waiting for password
        this.init();
    }

    async init() {
        await this.loadServers();
        await this.loadConfigInfo();
        this.setupEventListeners();
        this.setupToastContainer();
        this.startAutoRefresh();
        this.connectAllServers();
        this.updateConnectionStats();
    }

    setupEventListeners() {
        // Start monitoring on page load
        this.startMonitoring();
    }

    setupToastContainer() {
        // Create toast container if it doesn't exist
        if (!document.getElementById('toastContainer')) {
            const container = document.createElement('div');
            container.id = 'toastContainer';
            container.className = 'toast-container';
            document.body.appendChild(container);
        }
    }

    startAutoRefresh() {
        // Auto-refresh every 5 seconds to update timers and data
        this.monitoringInterval = setInterval(() => {
            this.loadServers();
            this.updateConnectionStats();
            this.updateMonitoringData();
        }, 5000);
    }

    startMonitoring() {
        this.isMonitoring = true;
    }

    async updateMonitoringData() {
        // Update monitoring data for all servers
        for (const server of this.servers) {
            try {
                const latestData = await this.fetchLatestMonitoringData(server.id);
                if (latestData) {
                    // Store in localStorage
                    this.storeMonitoringData(server.id, latestData);
                    // Update the server card UI
                    this.updateServerCard(server.id, latestData);
                }
            } catch (error) {
                console.error(`Error updating monitoring data for ${server.name}:`, error);
            }
        }
    }

    async fetchLatestMonitoringData(serverId) {
        try {
            // Fetch all monitoring data in parallel
            const [cpuResponse, memoryResponse, diskResponse] = await Promise.all([
                fetch(`/api/servers/${serverId}/details/cpu`),
                fetch(`/api/servers/${serverId}/details/memory`),
                fetch(`/api/servers/${serverId}/details/disks`)
            ]);

            const cpuData = cpuResponse.ok ? await cpuResponse.json() : null;
            const memoryData = memoryResponse.ok ? await memoryResponse.json() : null;
            const diskData = diskResponse.ok ? await diskResponse.json() : null;

            // Only return data if we have at least CPU data
            if (cpuData && !cpuData.error) {
                return {
                    cpu: cpuData,
                    memory: memoryData,
                    disks: diskData
                };
            }
        } catch (error) {
            console.error('Error fetching monitoring data:', error);
        }
        
        return null;
    }


    async loadServers() {
        try {
            const response = await fetch('/api/servers');
            if (!response.ok) throw new Error('Failed to load servers');
            
            this.servers = await response.json();
            await this.renderServers();
            this.updateNextMonitoringTimes();
        } catch (error) {
            console.error('Error loading servers:', error);
            this.showNotification('Failed to load servers', 'error');
        }
    }

    async loadConfigInfo() {
        try {
            const response = await fetch('/api/config-info');
            if (!response.ok) throw new Error('Failed to load config info');
            
            const configInfo = await response.json();
            this.updateConfigPath(configInfo.ssh_config_path);
        } catch (error) {
            console.error('Error loading config info:', error);
        }
    }

    updateConfigPath(sshConfigPath) {
        const sshInfoElement = document.querySelector('.ssh-info');
        if (sshInfoElement) {
            sshInfoElement.innerHTML = `<i class="fas fa-cog"></i> SSH Config: ${sshConfigPath}`;
        }
    }

    async renderServers() {
        const container = document.getElementById('serversContainer');
        
        if (!container) {
            console.error('serversContainer not found!');
            return;
        }
        
        container.innerHTML = '';

        if (this.servers.length === 0) {
            container.innerHTML = `
                <div class="text-center" style="padding: 3rem;">
                    <i class="fas fa-server" style="font-size: 4rem; color: #bdc3c7; margin-bottom: 1rem;"></i>
                    <h3 style="color: #7f8c8d; margin-bottom: 1rem;">No servers found</h3>
                    <p style="color: #95a5a6;">Configure your SSH config file to add servers</p>
                </div>
            `;
            return;
        }

        for (const server of this.servers) {
            const card = await this.createServerCard(server);
            container.appendChild(card);
        }

        // Animate all gauges with staggered timing
        setTimeout(() => {
            const allCircles = container.querySelectorAll('.circle-progress');
            allCircles.forEach((circle, index) => {
                setTimeout(() => {
                    // Force a reflow to ensure the CSS variables are applied
                    circle.offsetHeight;
                    circle.classList.add('animating');
                }, index * 150); // Stagger animation by 150ms per circle
            });
        }, 200);
    }

    async createServerCard(server) {
        const card = document.createElement('div');
        card.className = `server-card ${this.getStatusClass(server.status)}`;
        card.setAttribute('data-server-id', server.id);
        
        const statusText = this.getStatusText(server.status);
        const lastSeen = server.last_seen ? this.formatTimeAgo(new Date(server.last_seen)) : 'Never';

        // Get latest monitoring data for metrics
        const latestData = await this.getLatestMonitoringData(server.id);
        const cpuUsage = latestData?.cpu?.usage_percent || 0;
        const memoryUsage = latestData?.memory ? (latestData.memory.used / latestData.memory.total) * 100 : 0;
        const diskUsage = latestData?.disks ? this.calculateTotalDiskUsage(latestData.disks) : 0;

        // Calculate next monitoring time (display only, no countdown)
        const nextMonitoring = this.formatNextMonitoringTime(server.next_monitoring);
        const connectionStatus = this.serverConnections.get(server.id) || 'disconnected';
        const retryCount = this.serverRetryCount.get(server.id) || 0;

        card.innerHTML = `
            <div class="server-header">
                <div class="server-name">${server.name}</div>
                <div class="server-status ${this.getStatusClass(server.status)}">${statusText}</div>
            </div>
            <div class="server-metrics">
                <div class="metric-circle">
                    <div class="circle-progress" style="--color: ${this.getUsageColor(cpuUsage)}; --gauge-angle: ${(cpuUsage * 3.6)}deg">
                        <div class="circle-value">${cpuUsage.toFixed(1)}%</div>
                    </div>
                    <div class="circle-label">CPU</div>
                </div>
                <div class="metric-circle">
                    <div class="circle-progress" style="--color: ${this.getUsageColor(memoryUsage)}; --gauge-angle: ${(memoryUsage * 3.6)}deg">
                        <div class="circle-value">${memoryUsage.toFixed(1)}%</div>
                    </div>
                    <div class="circle-label">Memory</div>
                </div>
                <div class="metric-circle">
                    <div class="circle-progress" style="--color: ${this.getUsageColor(diskUsage)}; --gauge-angle: ${(diskUsage * 3.6)}deg">
                        <div class="circle-value">${diskUsage.toFixed(1)}%</div>
                    </div>
                    <div class="circle-label">Disk</div>
                </div>
            </div>
            <div class="server-info">
                <div class="server-info-item">
                    <span class="server-info-label">Host</span>
                    <span class="server-info-value">${server.host}:${server.port}</span>
                </div>
                <div class="server-info-item">
                    <span class="server-info-label">User</span>
                    <span class="server-info-value">${server.username}</span>
                </div>
                <div class="server-info-item">
                    <span class="server-info-label">Last Seen</span>
                    <span class="server-info-value">${lastSeen}</span>
                </div>
                <div class="server-info-item">
                    <span class="server-info-label">Next Check</span>
                    <span class="server-info-value" id="nextCheck-${server.id}">${nextMonitoring}</span>
                </div>
            </div>
            <div class="server-actions">
                <button class="btn btn-secondary btn-small" onclick="app.showServerDetails('${server.id}')">
                    <i class="fas fa-info-circle"></i> Details
                </button>
                ${retryCount >= 3 ? `<button class="btn btn-warning btn-small" onclick="app.retryConnection('${server.id}')">
                    <i class="fas fa-redo"></i> Retry
                </button>` : ''}
            </div>
        `;

        // Add smooth gauge animation on load
        setTimeout(() => {
            const circles = card.querySelectorAll('.circle-progress');
            circles.forEach(circle => {
                // Force a reflow to ensure the CSS variables are applied
                circle.offsetHeight;
                circle.classList.add('animating');
            });
        }, 100);

        return card;
    }

    async getLatestMonitoringData(serverId) {
        // Try to get from localStorage first
        const key = `monitoring_data_${serverId}`;
        const data = JSON.parse(localStorage.getItem(key) || '[]');
        if (data.length > 0) {
            return data[data.length - 1];
        }
        
        // If no local data, try to fetch from API
        return await this.fetchLatestMonitoringData(serverId);
    }

    formatNextMonitoringTime(nextMonitoring) {
        // Format next monitoring time for display (no countdown)
        if (nextMonitoring) {
            const now = new Date();
            const nextCheck = new Date(nextMonitoring * 1000); // Convert from Unix timestamp
            const diffMs = nextCheck - now;
            
            if (diffMs > 0) {
                const minutes = Math.floor(diffMs / 60000);
                const seconds = Math.floor((diffMs % 60000) / 1000);
                return `${minutes}:${seconds.toString().padStart(2, '0')}`;
            } else {
                return 'Now';
            }
        }
        return 'Unknown';
    }

    updateNextMonitoringTimes() {
        // Update next monitoring times in the UI
        for (const server of this.servers) {
            const nextCheckElement = document.getElementById(`nextCheck-${server.id}`);
            if (nextCheckElement) {
                nextCheckElement.textContent = this.formatNextMonitoringTime(server.next_monitoring);
            }
        }
    }

    // Password handling functions
    showPasswordModal(server) {
        this.pendingPasswordServer = server;
        document.getElementById('passwordServerName').textContent = server.name;
        document.getElementById('passwordInput').value = '';
        document.getElementById('rememberPassword').checked = true;
        document.getElementById('passwordModal').style.display = 'block';
        document.getElementById('passwordInput').focus();
    }

    closePasswordModal() {
        document.getElementById('passwordModal').style.display = 'none';
        this.pendingPasswordServer = null;
    }

    async submitPassword() {
        const password = document.getElementById('passwordInput').value;
        const remember = document.getElementById('rememberPassword').checked;
        
        if (!password) {
            this.showNotification('Please enter a password', 'error');
            return;
        }

        if (remember) {
            // Store password in localStorage
            const key = `ssh_password_${this.pendingPasswordServer.id}`;
            localStorage.setItem(key, password);
        }

        this.closePasswordModal();
        
        // Try to connect with the password
        try {
            await this.connectServerWithPassword(this.pendingPasswordServer.id, password);
            this.showNotification(`Connected to ${this.pendingPasswordServer.name}`, 'success');
        } catch (error) {
            this.showNotification(`Failed to connect to ${this.pendingPasswordServer.name}: ${error.message}`, 'error');
        }
    }

    async connectServerWithPassword(serverId, password) {
        const response = await fetch(`/api/servers/${serverId}/connect`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({ password })
        });

        if (!response.ok) {
            const error = await response.json();
            throw new Error(error.error || 'Connection failed');
        }

        const data = await response.json();
        this.serverConnections.set(serverId, 'connected');
        this.serverRetryCount.set(serverId, 0);
        
        return data;
    }

    getStoredPassword(serverId) {
        const key = `ssh_password_${serverId}`;
        return localStorage.getItem(key);
    }

    clearSecrets() {
        if (confirm('Are you sure you want to clear all stored passwords? This will require you to re-enter passwords for all servers.')) {
            // Clear all stored passwords
            for (let i = 0; i < localStorage.length; i++) {
                const key = localStorage.key(i);
                if (key && key.startsWith('ssh_password_')) {
                    localStorage.removeItem(key);
                }
            }
            this.showNotification('All stored passwords have been cleared', 'success');
        }
    }


    async connectAllServers() {
        // Connect to all servers on startup
        for (const server of this.servers) {
            await this.connectServer(server.id);
        }
    }

    async connectServer(serverId) {
        const server = this.servers.find(s => s.id === serverId);
        if (!server) return;

        // Check if already connected
        if (this.serverConnections.get(serverId) === 'connected') {
            return;
        }

        // Check if we have a pending request
        if (this.serverQueues.has(serverId)) {
            return;
        }

        // Add to queue
        this.serverQueues.set(serverId, true);

        try {
            // Check if we have a stored password
            const storedPassword = this.getStoredPassword(serverId);
            
            const response = await fetch(`/api/servers/${serverId}/connect`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify({ password: storedPassword })
            });

            const result = await response.json();
            
            if (result.status === 'connected') {
                this.serverConnections.set(serverId, 'connected');
                this.serverRetryCount.set(serverId, 0);
                this.showNotification(`Connected to ${server.name}`, 'success');
            } else {
                // Check if it's a password error
                if (result.message && (result.message.includes('password') || result.message.includes('authentication'))) {
                    this.showPasswordModal(server);
                } else {
                    this.serverConnections.set(serverId, 'error');
                    this.showNotification(`Connection failed: ${result.message}`, 'error');
                    
                    // Increment retry count
                    const retryCount = this.serverRetryCount.get(serverId) || 0;
                    this.serverRetryCount.set(serverId, retryCount + 1);
                }
            }
            
            // Re-render to update UI
            await this.renderServers();
        } catch (error) {
            console.error('Error connecting to server:', error);
            this.serverConnections.set(serverId, 'error');
            this.showNotification('Failed to connect to server', 'error');
        } finally {
            // Remove from queue
            this.serverQueues.delete(serverId);
        }
    }

    async retryConnection(serverId) {
        // Reset retry count and try again
        this.serverRetryCount.set(serverId, 0);
        await this.connectServer(serverId);
    }



    isRetryableError(error) {
        const retryableErrors = [
            'timeout',
            'connection refused',
            'network unreachable',
            'host unreachable',
            'connection reset',
            'broken pipe'
        ];
        
        return retryableErrors.some(retryableError => 
            error.toLowerCase().includes(retryableError)
        );
    }

    storeMonitoringData(serverId, data) {
        // Store in localStorage for persistence
        const key = `monitoring_data_${serverId}`;
        const existingData = JSON.parse(localStorage.getItem(key) || '[]');
        existingData.push({
            ...data,
            timestamp: new Date().toISOString()
        });
        
        // Keep only last 100 entries
        if (existingData.length > 100) {
            existingData.splice(0, existingData.length - 100);
        }
        
        localStorage.setItem(key, JSON.stringify(existingData));
    }

    getLatestMonitoringData(serverId) {
        const key = `monitoring_data_${serverId}`;
        const data = JSON.parse(localStorage.getItem(key) || '[]');
        return data.length > 0 ? data[data.length - 1] : null;
    }

    updateServerCard(serverId, data) {
        // Update the metrics in the server card
        const card = document.querySelector(`[data-server-id="${serverId}"]`);
        if (!card) return;

        const cpuUsage = data.cpu?.usage_percent || 0;
        const memoryUsage = data.memory ? (data.memory.used / data.memory.total) * 100 : 0;
        const diskUsage = data.disks ? this.calculateTotalDiskUsage(data.disks) : 0;

        // Update CPU metric
        const cpuCircle = card.querySelector('.metric-circle:nth-child(1) .circle-progress');
        const cpuValue = card.querySelector('.metric-circle:nth-child(1) .circle-value');
        if (cpuCircle && cpuValue) {
            cpuCircle.style.setProperty('--color', this.getUsageColor(cpuUsage));
            cpuCircle.style.setProperty('--gauge-angle', `${(cpuUsage * 3.6)}deg`);
            cpuValue.textContent = `${cpuUsage.toFixed(1)}%`;
            // Trigger animation
            cpuCircle.classList.remove('animating');
            setTimeout(() => cpuCircle.classList.add('animating'), 10);
        }

        // Update Memory metric
        const memoryCircle = card.querySelector('.metric-circle:nth-child(2) .circle-progress');
        const memoryValue = card.querySelector('.metric-circle:nth-child(2) .circle-value');
        if (memoryCircle && memoryValue) {
            memoryCircle.style.setProperty('--color', this.getUsageColor(memoryUsage));
            memoryCircle.style.setProperty('--gauge-angle', `${(memoryUsage * 3.6)}deg`);
            memoryValue.textContent = `${memoryUsage.toFixed(1)}%`;
            // Trigger animation
            memoryCircle.classList.remove('animating');
            setTimeout(() => memoryCircle.classList.add('animating'), 10);
        }

        // Update Disk metric
        const diskCircle = card.querySelector('.metric-circle:nth-child(3) .circle-progress');
        const diskValue = card.querySelector('.metric-circle:nth-child(3) .circle-value');
        if (diskCircle && diskValue) {
            diskCircle.style.setProperty('--color', this.getUsageColor(diskUsage));
            diskCircle.style.setProperty('--gauge-angle', `${(diskUsage * 3.6)}deg`);
            diskValue.textContent = `${diskUsage.toFixed(1)}%`;
            // Trigger animation
            diskCircle.classList.remove('animating');
            setTimeout(() => diskCircle.classList.add('animating'), 10);
        }
    }

    calculateTotalDiskUsage(disks) {
        if (!Array.isArray(disks) || disks.length === 0) return 0;
        
        // Calculate weighted average based on disk size
        let totalUsed = 0;
        let totalSize = 0;
        
        for (const disk of disks) {
            if (disk.used && disk.total) {
                totalUsed += disk.used;
                totalSize += disk.total;
            }
        }
        
        if (totalSize === 0) return 0;
        return (totalUsed / totalSize) * 100;
    }

    getUsageColor(usage) {
        const numUsage = Number(usage) || 0;
        if (numUsage >= 80) return '#dc3545';
        if (numUsage >= 60) return '#ffc107';
        return '#28a745';
    }

    getStatusClass(status) {
        if (typeof status === 'string') {
            switch (status) {
                case 'Online': return 'status-online';
                case 'Offline': return 'status-offline';
                case 'Connecting': return 'status-connecting';
                default: return 'status-error';
            }
        }
        if (typeof status === 'object' && status.Online) return 'status-online';
        if (typeof status === 'object' && status.Offline) return 'status-offline';
        if (typeof status === 'object' && status.Error) return 'status-error';
        return 'status-offline';
    }

    getStatusText(status) {
        if (typeof status === 'string') {
            return status;
        }
        if (typeof status === 'object' && status.Online) return 'Online';
        if (typeof status === 'object' && status.Offline) return 'Offline';
        if (typeof status === 'object' && status.Error) return 'Error';
        return 'Unknown';
    }

    async showServerDetails(serverId) {
        this.currentServer = this.servers.find(s => s.id === serverId);
        document.getElementById('serverDetailsTitle').textContent = `${this.currentServer.name} - Details`;
        
        // Show modal immediately with cached data
        document.getElementById('serverDetailsModal').style.display = 'block';
        
        // Load overview tab with cached data (no loading needed)
        await this.loadServerDetails('overview');
    }

    async loadServerDetails(tab) {
        const content = document.getElementById('tabContent');
        // Hide all tabs
        document.querySelectorAll('.tab-button').forEach(btn => btn.classList.remove('active'));
        event?.target?.classList?.add('active');

        // Show loading spinner/message
        content.innerHTML = `<div class="loading-container"><div class="loading-spinner"></div><div>Loading ${tab} data...</div></div>`;

        try {
            if (tab === 'overview') {
                content.innerHTML = this.renderOverviewTab();
            } else if (tab === 'history') {
                content.innerHTML = await this.renderHistoryTab();
            } else {
                const response = await fetch(`/api/servers/${this.currentServer.id}/details/${tab}`);
                if (!response.ok) throw new Error(`HTTP ${response.status}`);
                const data = await response.json();
                content.innerHTML = this.renderDetailsTab(tab, data);
            }
        } catch (error) {
            content.innerHTML = `<div class='error-container'><i class='fas fa-exclamation-triangle'></i><p style='font-size:1.1em;'>Error loading ${tab} data:</p><div style='margin-top:0.5em;'>${error.message}</div></div>`;
        }
    }

    renderOverviewTab() {
        return `
            <div class="metric-grid">
                <div class="metric-card">
                    <div class="metric-title">Server Status</div>
                    <div class="metric-value">${this.getStatusText(this.currentServer.status)}</div>
                </div>
                <div class="metric-card">
                    <div class="metric-title">Host</div>
                    <div class="metric-value">${this.currentServer.host}:${this.currentServer.port}</div>
                </div>
                <div class="metric-card">
                    <div class="metric-title">Username</div>
                    <div class="metric-value">${this.currentServer.username}</div>
                </div>
                <div class="metric-card">
                    <div class="metric-title">Last Seen</div>
                    <div class="metric-value">${this.currentServer.last_seen ? this.formatTimeAgo(new Date(this.currentServer.last_seen)) : 'Never'}</div>
                </div>
            </div>
        `;
    }

    async renderHistoryTab() {
        try {
            const response = await fetch(`/api/servers/${this.currentServer.id}/history?limit=50`);
            const historicalData = await response.json();
            
            if (historicalData.length === 0) {
                return '<p>No historical data available. Start monitoring to see data here.</p>';
            }

            return `
                <div class="history-container">
                    <h3>Monitoring History (Last 50 entries)</h3>
                    <div class="history-chart">
                        ${this.renderHistoryChart(historicalData)}
                    </div>
                </div>
            `;
        } catch (error) {
            return `<p>Error loading history: ${error.message}</p>`;
        }
    }

    renderHistoryChart(data) {
        if (data.length === 0) return '<p>No data to display</p>';
        
        const maxCpu = Math.max(...data.map(d => d.cpu?.usage_percent || 0));
        const maxMemory = Math.max(...data.map(d => d.memory ? (d.memory.used / d.memory.total) * 100 : 0));
        
        return `
            <div class="chart-container">
                <div class="chart">
                    <h4>CPU Usage Over Time</h4>
                    <div class="chart-bars">
                        ${data.slice(-20).map((d, i) => {
                            const height = maxCpu > 0 ? ((d.cpu?.usage_percent || 0) / maxCpu) * 100 : 0;
                            return `<div class="chart-bar" style="height: ${height}%"></div>`;
                        }).join('')}
                    </div>
                </div>
                <div class="chart">
                    <h4>Memory Usage Over Time</h4>
                    <div class="chart-bars">
                        ${data.slice(-20).map((d, i) => {
                            const height = maxMemory > 0 ? ((d.memory ? (d.memory.used / d.memory.total) * 100 : 0) / maxMemory) * 100 : 0;
                            return `<div class="chart-bar" style="height: ${height}%"></div>`;
                        }).join('')}
                    </div>
                </div>
            </div>
        `;
    }

    renderDetailsTab(tab, data) {
        // If there's an error in the response, show a prominent message
        if (data.error) {
            return `<div class='error-container'><i class='fas fa-exclamation-circle'></i><p>Data unavailable or error for this metric:</p><div style='margin-top:0.5em;'>${data.error}</div></div>`;
        }
        if (tab === 'cpu') {
            if (data.cores === 0) return `<div class='error-container'><i class='fas fa-exclamation-circle'></i><p>CPU info unavailable.</p></div>`;
            return `
            <div class="metric-grid">
                <div class="metric-card">
                    <div class="metric-title">CPU Usage</div>
                    <div class="metric-value">${data.usage_percent.toFixed(1)}%</div>
                    <div class="progress-bar">
                        <div class="progress-fill ${data.usage_percent > 80 ? 'danger' : data.usage_percent > 60 ? 'warning' : ''}" 
                             style="width: ${data.usage_percent}%"></div>
                    </div>
                </div>
                <div class="metric-card">
                    <div class="metric-title">Load Average (1m)</div>
                    <div class="metric-value">${data.load_average[0].toFixed(2)}</div>
                </div>
                <div class="metric-card">
                    <div class="metric-title">Load Average (5m)</div>
                    <div class="metric-value">${data.load_average[1].toFixed(2)}</div>
                </div>
                <div class="metric-card">
                    <div class="metric-title">Load Average (15m)</div>
                    <div class="metric-value">${data.load_average[2].toFixed(2)}</div>
                </div>
                <div class="metric-card">
                    <div class="metric-title">CPU Cores</div>
                    <div class="metric-value">${data.cores}</div>
                </div>
                <div class="metric-card">
                    <div class="metric-title">CPU Model</div>
                    <div class="metric-value">${data.model}</div>
                </div>
            </div>
        `;
        }
        if (tab === 'memory') {
            if (!data.total) return `<div class='error-container'><i class='fas fa-exclamation-circle'></i><p>Memory info unavailable.</p></div>`;
            const totalGB = (data.total / (1024 * 1024 * 1024)).toFixed(2);
            const usedGB = (data.used / (1024 * 1024 * 1024)).toFixed(2);
            const freeGB = (data.free / (1024 * 1024 * 1024)).toFixed(2);
            const usagePercent = ((data.used / data.total) * 100).toFixed(1);

            return `
            <div class="metric-grid">
                <div class="metric-card">
                    <div class="metric-title">Total Memory</div>
                    <div class="metric-value">${totalGB} GB</div>
                </div>
                <div class="metric-card">
                    <div class="metric-title">Used Memory</div>
                    <div class="metric-value">${usedGB} GB</div>
                </div>
                <div class="metric-card">
                    <div class="metric-title">Free Memory</div>
                    <div class="metric-value">${freeGB} GB</div>
                </div>
                <div class="metric-card">
                    <div class="metric-title">Memory Usage</div>
                    <div class="metric-value">${usagePercent}%</div>
                    <div class="progress-bar">
                        <div class="progress-fill ${usagePercent > 80 ? 'danger' : usagePercent > 60 ? 'warning' : ''}" 
                             style="width: ${usagePercent}%"></div>
                    </div>
                </div>
                <div class="metric-card">
                    <div class="metric-title">Swap Total</div>
                    <div class="metric-value">${(data.swap_total / (1024 * 1024 * 1024)).toFixed(2)} GB</div>
                </div>
                <div class="metric-card">
                    <div class="metric-title">Swap Used</div>
                    <div class="metric-value">${(data.swap_used / (1024 * 1024 * 1024)).toFixed(2)} GB</div>
                </div>
            </div>
        `;
        }
        if (tab === 'disks') {
            if (!Array.isArray(data) || !data.length) return `<div class='error-container'><i class='fas fa-exclamation-circle'></i><p>No disk info available.</p></div>`;
            return `
            <div class="metric-grid">
                ${data.map(disk => `
                    <div class="metric-card">
                        <div class="metric-title">${disk.device} (${disk.mount_point})</div>
                        <div class="metric-value">${disk.usage_percent.toFixed(1)}%</div>
                        <div class="progress-bar">
                            <div class="progress-fill ${disk.usage_percent > 80 ? 'danger' : disk.usage_percent > 60 ? 'warning' : ''}" 
                                 style="width: ${disk.usage_percent}%"></div>
                        </div>
                        <div style="margin-top: 0.5rem; font-size: 0.8rem; color: #7f8c8d;">
                            ${(disk.used / (1024 * 1024 * 1024)).toFixed(1)} GB / ${(disk.total / (1024 * 1024 * 1024)).toFixed(1)} GB
                        </div>
                        <div style="font-size: 0.8rem; color: #95a5a6;">
                            ${disk.filesystem}
                        </div>
                    </div>
                `).join('')}
            </div>
        `;
        }
        if (tab === 'network') {
            if (!data || data.length === 0) {
                return '<p>No network information available</p>';
            }

            return `
                <div class="metric-grid">
                    ${data.map(net => `
                        <div class="metric-card">
                            <div class="metric-title">${net.interface}</div>
                            <div class="metric-value">RX: ${(net.rx_bytes / (1024 * 1024)).toFixed(1)} MB</div>
                            <div class="metric-value">TX: ${(net.tx_bytes / (1024 * 1024)).toFixed(1)} MB</div>
                            <div style="margin-top: 0.5rem; font-size: 0.8rem; color: #7f8c8d;">
                                Packets: ${net.rx_packets.toLocaleString()} RX, ${net.tx_packets.toLocaleString()} TX
                            </div>
                        </div>
                    `).join('')}
                </div>
            `;
        }
        if (tab === 'ports') {
            if (!data || data.length === 0) {
                return '<p>No port information available</p>';
            }

            return `
                <div class="metric-grid">
                    ${data.map(port => `
                        <div class="metric-card">
                            <div class="metric-title">Port ${port.port}</div>
                            <div class="metric-value">${port.protocol.toUpperCase()}</div>
                            <div style="margin-top: 0.5rem; font-size: 0.8rem; color: #7f8c8d;">
                                State: ${port.state}
                            </div>
                        </div>
                    `).join('')}
                </div>
            `;
        }
        if (tab === 'ping') {
            const pings = Array.isArray(data) ? data : [];
            let valid = pings.filter(p => p.target && (!p.error || p.success));
            if (valid.length === 0) return `<div class='error-container'><i class='fas fa-exclamation-circle'></i><p>No ping data available.</p></div>`;
            return `
            <div class="metric-grid">
                ${valid.map(ping => `
                    <div class="metric-card">
                        <div class="metric-title">${ping.target}</div>
                        <div class="metric-value">${ping.success ? `${ping.latency_ms?.toFixed(1) || 'N/A'} ms` : 'Failed'}</div>
                        <div style="margin-top: 0.5rem; font-size: 0.8rem; color: ${ping.success ? '#27ae60' : '#e74c3c'};">
                            ${ping.success ? 'Success' : ping.error || 'Unknown error'}
                        </div>
                    </div>
                `).join('')}
            </div>
        `;
        }
        if (tab === 'system') {
            return `
            <div class="metric-grid">
                <div class="metric-card">
                    <div class="metric-title">Hostname</div>
                    <div class="metric-value">${data.hostname}</div>
                </div>
                <div class="metric-card">
                    <div class="metric-title">Operating System</div>
                    <div class="metric-value">${data.os}</div>
                </div>
                <div class="metric-card">
                    <div class="metric-title">Kernel</div>
                    <div class="metric-value">${data.kernel}</div>
                </div>
                <div class="metric-card">
                    <div class="metric-title">Architecture</div>
                    <div class="metric-value">${data.architecture}</div>
                </div>
                <div class="metric-card">
                    <div class="metric-title">Uptime</div>
                    <div class="metric-value">${Math.floor(data.uptime / 3600)} hours</div>
                </div>
            </div>
        `;
        }
        return `<p>Unknown tab: ${tab}</p>`;
    }

    renderCpuTab(data) {
        return `
            <div class="metric-grid">
                <div class="metric-card">
                    <div class="metric-title">CPU Usage</div>
                    <div class="metric-value">${data.usage_percent.toFixed(1)}%</div>
                    <div class="progress-bar">
                        <div class="progress-fill ${data.usage_percent > 80 ? 'danger' : data.usage_percent > 60 ? 'warning' : ''}" 
                             style="width: ${data.usage_percent}%"></div>
                    </div>
                </div>
                <div class="metric-card">
                    <div class="metric-title">Load Average (1m)</div>
                    <div class="metric-value">${data.load_average[0].toFixed(2)}</div>
                </div>
                <div class="metric-card">
                    <div class="metric-title">Load Average (5m)</div>
                    <div class="metric-value">${data.load_average[1].toFixed(2)}</div>
                </div>
                <div class="metric-card">
                    <div class="metric-title">Load Average (15m)</div>
                    <div class="metric-value">${data.load_average[2].toFixed(2)}</div>
                </div>
                <div class="metric-card">
                    <div class="metric-title">CPU Cores</div>
                    <div class="metric-value">${data.cores}</div>
                </div>
                <div class="metric-card">
                    <div class="metric-title">CPU Model</div>
                    <div class="metric-value">${data.model}</div>
                </div>
            </div>
        `;
    }

    renderMemoryTab(data) {
        const totalGB = (data.total / (1024 * 1024 * 1024)).toFixed(2);
        const usedGB = (data.used / (1024 * 1024 * 1024)).toFixed(2);
        const freeGB = (data.free / (1024 * 1024 * 1024)).toFixed(2);
        const usagePercent = ((data.used / data.total) * 100).toFixed(1);

        return `
            <div class="metric-grid">
                <div class="metric-card">
                    <div class="metric-title">Total Memory</div>
                    <div class="metric-value">${totalGB} GB</div>
                </div>
                <div class="metric-card">
                    <div class="metric-title">Used Memory</div>
                    <div class="metric-value">${usedGB} GB</div>
                </div>
                <div class="metric-card">
                    <div class="metric-title">Free Memory</div>
                    <div class="metric-value">${freeGB} GB</div>
                </div>
                <div class="metric-card">
                    <div class="metric-title">Memory Usage</div>
                    <div class="metric-value">${usagePercent}%</div>
                    <div class="progress-bar">
                        <div class="progress-fill ${usagePercent > 80 ? 'danger' : usagePercent > 60 ? 'warning' : ''}" 
                             style="width: ${usagePercent}%"></div>
                    </div>
                </div>
                <div class="metric-card">
                    <div class="metric-title">Swap Total</div>
                    <div class="metric-value">${(data.swap_total / (1024 * 1024 * 1024)).toFixed(2)} GB</div>
                </div>
                <div class="metric-card">
                    <div class="metric-title">Swap Used</div>
                    <div class="metric-value">${(data.swap_used / (1024 * 1024 * 1024)).toFixed(2)} GB</div>
                </div>
            </div>
        `;
    }

    renderDisksTab(data) {
        if (!data || data.length === 0) {
            return '<p>No disk information available</p>';
        }

        return `
            <div class="metric-grid">
                ${data.map(disk => `
                    <div class="metric-card">
                        <div class="metric-title">${disk.device} (${disk.mount_point})</div>
                        <div class="metric-value">${disk.usage_percent.toFixed(1)}%</div>
                        <div class="progress-bar">
                            <div class="progress-fill ${disk.usage_percent > 80 ? 'danger' : disk.usage_percent > 60 ? 'warning' : ''}" 
                                 style="width: ${disk.usage_percent}%"></div>
                        </div>
                        <div style="margin-top: 0.5rem; font-size: 0.8rem; color: #7f8c8d;">
                            ${(disk.used / (1024 * 1024 * 1024)).toFixed(1)} GB / ${(disk.total / (1024 * 1024 * 1024)).toFixed(1)} GB
                        </div>
                        <div style="font-size: 0.8rem; color: #95a5a6;">
                            ${disk.filesystem}
                        </div>
                    </div>
                `).join('')}
            </div>
        `;
    }

    renderNetworkTab(data) {
        if (!data || data.length === 0) {
            return '<p>No network information available</p>';
        }

        return `
            <div class="metric-grid">
                ${data.map(net => `
                    <div class="metric-card">
                        <div class="metric-title">${net.interface}</div>
                        <div class="metric-value">RX: ${(net.rx_bytes / (1024 * 1024)).toFixed(1)} MB</div>
                        <div class="metric-value">TX: ${(net.tx_bytes / (1024 * 1024)).toFixed(1)} MB</div>
                        <div style="margin-top: 0.5rem; font-size: 0.8rem; color: #7f8c8d;">
                            Packets: ${net.rx_packets.toLocaleString()} RX, ${net.tx_packets.toLocaleString()} TX
                        </div>
                    </div>
                `).join('')}
            </div>
        `;
    }

    renderPortsTab(data) {
        if (!data || data.length === 0) {
            return '<p>No port information available</p>';
        }

        return `
            <div class="metric-grid">
                ${data.map(port => `
                    <div class="metric-card">
                        <div class="metric-title">Port ${port.port}</div>
                        <div class="metric-value">${port.protocol.toUpperCase()}</div>
                        <div style="margin-top: 0.5rem; font-size: 0.8rem; color: #7f8c8d;">
                            State: ${port.state}
                        </div>
                    </div>
                `).join('')}
            </div>
        `;
    }

    renderPingTab(data) {
        if (!data || data.length === 0) {
            return '<p>No ping test data available</p>';
        }

        return `
            <div class="metric-grid">
                ${data.map(ping => `
                    <div class="metric-card">
                        <div class="metric-title">${ping.target}</div>
                        <div class="metric-value">${ping.success ? `${ping.latency_ms?.toFixed(1) || 'N/A'} ms` : 'Failed'}</div>
                        <div style="margin-top: 0.5rem; font-size: 0.8rem; color: ${ping.success ? '#27ae60' : '#e74c3c'};">
                            ${ping.success ? 'Success' : ping.error || 'Unknown error'}
                        </div>
                    </div>
                `).join('')}
            </div>
        `;
    }

    renderSystemTab(data) {
        return `
            <div class="metric-grid">
                <div class="metric-card">
                    <div class="metric-title">Hostname</div>
                    <div class="metric-value">${data.hostname}</div>
                </div>
                <div class="metric-card">
                    <div class="metric-title">Operating System</div>
                    <div class="metric-value">${data.os}</div>
                </div>
                <div class="metric-card">
                    <div class="metric-title">Kernel</div>
                    <div class="metric-value">${data.kernel}</div>
                </div>
                <div class="metric-card">
                    <div class="metric-title">Architecture</div>
                    <div class="metric-value">${data.architecture}</div>
                </div>
                <div class="metric-card">
                    <div class="metric-title">Uptime</div>
                    <div class="metric-value">${Math.floor(data.uptime / 3600)} hours</div>
                </div>
            </div>
        `;
    }

    showNotification(message, type = 'info') {
        const container = document.getElementById('toastContainer');
        
        // Create notification element
        const notification = document.createElement('div');
        notification.className = `notification notification-${type}`;
        notification.textContent = message;

        container.appendChild(notification);

        // Remove after 4 seconds
        setTimeout(() => {
            notification.style.animation = 'slideOutRight 0.3s ease';
            setTimeout(() => {
                if (container.contains(notification)) {
                    container.removeChild(notification);
                }
            }, 300);
        }, 4000);
    }

    // Modal functions
    closeServerDetailsModal() {
        document.getElementById('serverDetailsModal').style.display = 'none';
    }

    async showConnectionPoolDetails() {
        document.getElementById('connectionPoolModal').style.display = 'block';
        
        try {
            const response = await fetch('/api/connection-pool');
            if (!response.ok) throw new Error('Failed to load connection pool details');
            
            const data = await response.json();
            this.renderConnectionPoolDetails(data);
        } catch (error) {
            console.error('Error loading connection pool details:', error);
            document.getElementById('connectionPoolContent').innerHTML = `
                <div class="error-container">
                    <i class="fas fa-exclamation-triangle"></i>
                    <p>Error loading connection pool details: ${error.message}</p>
                </div>
            `;
        }
    }

    closeConnectionPoolModal() {
        document.getElementById('connectionPoolModal').style.display = 'none';
    }

    renderConnectionPoolDetails(data) {
        const formatAge = (seconds) => {
            if (seconds === 0) return 'Now';
            if (seconds < 60) return `${seconds}s ago`;
            if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`;
            return `${Math.floor(seconds / 3600)}h ago`;
        };

        const formatNextMonitoring = (seconds) => {
            if (seconds === 0) return 'Now';
            if (seconds < 60) return `in ${seconds}s`;
            if (seconds < 3600) return `in ${Math.floor(seconds / 60)}m`;
            return `in ${Math.floor(seconds / 3600)}h`;
        };

        const getStatusClass = (status) => {
            switch (status) {
                case 'Online': return 'status-online';
                case 'Offline': return 'status-offline';
                case 'Connecting': return 'status-connecting';
                case 'Error': return 'status-error';
                default: return 'status-offline';
            }
        };

        document.getElementById('connectionPoolContent').innerHTML = `
            <div class="connection-pool-details">
                <div class="summary-section">
                    <h3>Summary</h3>
                    <div class="metric-grid">
                        <div class="metric-card">
                            <div class="metric-title">Total Servers</div>
                            <div class="metric-value">${data.summary.total_servers}</div>
                        </div>
                        <div class="metric-card">
                            <div class="metric-title">Online</div>
                            <div class="metric-value">${data.summary.online_servers}</div>
                        </div>
                        <div class="metric-card">
                            <div class="metric-title">Offline</div>
                            <div class="metric-value">${data.summary.offline_servers}</div>
                        </div>
                        <div class="metric-card">
                            <div class="metric-title">Error</div>
                            <div class="metric-value">${data.summary.error_servers}</div>
                        </div>
                        <div class="metric-card">
                            <div class="metric-title">Connecting</div>
                            <div class="metric-value">${data.summary.connecting_servers}</div>
                        </div>
                    </div>
                </div>

                <div class="ssh-pool-section">
                    <h3>SSH Connection Pool</h3>
                    <div class="metric-grid">
                        <div class="metric-card">
                            <div class="metric-title">Active SSH Connections</div>
                            <div class="metric-value">${data.ssh_connection_pool.active_connections}</div>
                        </div>
                        <div class="metric-card">
                            <div class="metric-title">Total SSH Connections</div>
                            <div class="metric-value">${data.ssh_connection_pool.total_connections}</div>
                        </div>
                    </div>
                </div>

                <div class="server-connections-section">
                    <h3>Server Connections</h3>
                    <div class="server-connections-list">
                        ${data.server_connections.map(server => `
                            <div class="server-connection-item">
                                <div class="server-connection-header">
                                    <div class="server-name">${server.server_name}</div>
                                    <div class="server-status ${getStatusClass(server.status)}">${server.status}</div>
                                </div>
                                <div class="server-connection-details">
                                    <div class="detail-row">
                                        <span class="detail-label">Host:</span>
                                        <span class="detail-value">${server.host}</span>
                                    </div>
                                    <div class="detail-row">
                                        <span class="detail-label">Username:</span>
                                        <span class="detail-value">${server.username}</span>
                                    </div>
                                    <div class="detail-row">
                                        <span class="detail-label">Last Seen:</span>
                                        <span class="detail-value">${formatAge(server.last_seen_age_seconds)}</span>
                                    </div>
                                    <div class="detail-row">
                                        <span class="detail-label">Next Monitoring:</span>
                                        <span class="detail-value">${formatNextMonitoring(server.next_monitoring_age_seconds)}</span>
                                    </div>
                                    <div class="detail-row">
                                        <span class="detail-label">Monitoring Interval:</span>
                                        <span class="detail-value">${server.monitoring_interval_seconds}s</span>
                                    </div>
                                    <div class="detail-row">
                                        <span class="detail-label">SSH Connection:</span>
                                        <span class="detail-value ${server.has_ssh_connection ? 'status-online' : 'status-offline'}">
                                            ${server.has_ssh_connection ? 'Active' : 'Inactive'}
                                        </span>
                                    </div>
                                </div>
                            </div>
                        `).join('')}
                    </div>
                </div>
            </div>
        `;
    }

    formatTimeAgo(date) {
        const now = new Date();
        const diffInSeconds = Math.floor((now - date) / 1000);
        
        if (diffInSeconds < 60) {
            return `${diffInSeconds}s ago`;
        } else if (diffInSeconds < 3600) {
            const minutes = Math.floor(diffInSeconds / 60);
            return `${minutes}m ago`;
        } else if (diffInSeconds < 86400) {
            const hours = Math.floor(diffInSeconds / 3600);
            return `${hours}h ago`;
        } else {
            const days = Math.floor(diffInSeconds / 86400);
            return `${days}d ago`;
        }
    }

    async updateConnectionStats() {
        try {
            const response = await fetch('/api/connection-stats');
            if (!response.ok) return;
            
            const stats = await response.json();
            
            document.getElementById('activeConnections').textContent = stats.active_connections;
            
            const formatAge = (seconds) => {
                if (seconds === 0) return '-';
                if (seconds < 60) return `${seconds}s`;
                if (seconds < 3600) return `${Math.floor(seconds / 60)}m`;
                return `${Math.floor(seconds / 3600)}h`;
            };
            
            document.getElementById('oldestConnection').textContent = formatAge(stats.oldest_connection_age_seconds);
            document.getElementById('youngestConnection').textContent = formatAge(stats.youngest_connection_age_seconds);
        } catch (error) {
            console.error('Failed to update connection stats:', error);
        }
    }
}

// Global functions for HTML onclick handlers
function closeServerDetailsModal() {
    app.closeServerDetailsModal();
}

function closePasswordModal() {
    app.closePasswordModal();
}

function submitPassword() {
    app.submitPassword();
}

function clearSecrets() {
    app.clearSecrets();
}

function showTab(tabName) {
    app.loadServerDetails(tabName);
}

function closeConnectionPoolModal() {
    app.closeConnectionPoolModal();
}

// Initialize app when DOM is loaded
let app;
document.addEventListener('DOMContentLoaded', () => {
    app = new MonitorApp();
});