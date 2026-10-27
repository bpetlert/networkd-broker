# Notes

## Monitoring Network Event

```bash
dbus-monitor --system "type='signal',path_namespace='/org/freedesktop/network1/link',interface='org.freedesktop.DBus.Properties',member='PropertiesChanged'"
```

Or

```bash
sudo busctl monitor --match "type='signal',path_namespace='/org/freedesktop/network1/link',interface='org.freedesktop.DBus.Properties',member='PropertiesChanged'"
```

## Get all links

```bash
busctl --system call org.freedesktop.network1 /org/freedesktop/network1 org.freedesktop.network1.Manager ListLinks
```

## Generate D-Bus interface using zbus-xmlgen 6.0.0

```bash
zbus-xmlgen system org.freedesktop.network1 /org/freedesktop/network1
```
