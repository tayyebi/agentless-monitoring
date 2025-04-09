To run this app:

```
python3 manage.py runserver
```

Server: `/usr/local/bin/log-usage`:

```
#!/bin/bash

# Function to get network upload and download
get_network_data() {
    # Adjust the following lines to match your system/network interface
    network_data=$(cat /proc/net/dev | grep eth0 | awk '{print $2, $10}')
    network_upload=$(echo $network_data | awk '{print $1}')
    network_download=$(echo $network_data | awk '{print $2}')
}

# Function to get percentage of disk usage
get_disk_usage() {
    disk_usage=$(df -h / | awk 'NR==2 {print $5}')
}

# Function to get CPU percentage
get_cpu_percentage() {
    cpu_percentage=$(top -bn1 | grep "Cpu(s)" | sed "s/.*, *\([0-9.]*\)%* id.*/\1/" | awk '{print 100 - $1}')
}

# Function to get RAM percentage
get_ram_percentage() {
    ram_percentage=$(free | grep Mem | awk '{print $3/$2 * 100.0}')
}

# Function to get current date and time
get_datetime() {
    datetime=$(date +"%Y-%m-%d %H:%M:%S")
}

# Fetch all metrics
get_network_data
get_disk_usage
get_cpu_percentage
get_ram_percentage
get_datetime

# Display the data
echo "$datetime, upload $network_upload KB, download $network_download KB, disk $disk_usage, cpu $cpu_percentage %, ram $ram_percentage %"
```