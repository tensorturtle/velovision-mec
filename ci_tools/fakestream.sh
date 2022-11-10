# Create a mock video stream for testing purposes

# required packages on Debian/Ubuntu
# v4l2loopback: https://github.com/umlaeute/v4l2loopback
# gstreamer: https://gstreamer.freedesktop.org/documentation/installing/on-linux.html?gi-language=c

# apt-get commands:
# apt-get install v4l2loopback-dkms
# apt-get install libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libgstreamer-plugins-bad1.0-dev gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly gstreamer1.0-libav gstreamer1.0-doc gstreamer1.0-tools gstreamer1.0-x gstreamer1.0-alsa gstreamer1.0-gl gstreamer1.0-gtk3 gstreamer1.0-qt5 gstreamer1.0-pulseaudio

VIDEO_DEV_NUM=$1

# Get command line args
if [ -z "$VIDEO_DEV_NUM" ]; then
    echo "Usage: fakestream.sh <video device number>"
    echo "Example: fakestream.sh 0"
    exit 1
fi

# stop existing video streams
sudo killall gst-launch-1.0

# if any video device is already using the v4l2loopback module, unload it (remove virtual cameras)
sudo modprobe -r v4l2loopback

if [ -n "$(ls /dev/video*)" ]; then
    echo "WARNING: video devices already connected"
    echo
    #exit 1
fi

# Use v4l2loopback to create a virtual video device
sudo modprobe v4l2loopback video_nr=$VIDEO_DEV_NUM

# start streaming of video test pattern
gst-launch-1.0 videotestsrc pattern=smpte ! video/x-raw,width=640,height=480,framerate=30/1 ! v4l2sink device=/dev/video$VIDEO_DEV_NUM &

sleep 0.3
echo
echo "Test stream running on /dev/video$VIDEO_DEV_NUM"
echo "To stop the video test pattern, run: sudo killall gst-launch-1.0"
