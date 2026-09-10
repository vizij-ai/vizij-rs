# ROS 2 Jazzy with rmw_zenoh and the ROS4HRI skill interfaces built from the
# definitions arora-msgs-ros2 vendors — the peer a native client needs to speak
# /skill/say. Built by build-image.sh, which stages the interface packages.
FROM ros:jazzy-ros-base
RUN apt-get update && apt-get install -y --no-install-recommends \
    ros-jazzy-rmw-zenoh-cpp ros-jazzy-rosidl-default-generators ros-jazzy-action-msgs \
    python3-colcon-common-extensions python3-numpy build-essential \
    && rm -rf /var/lib/apt/lists/*
COPY ros4hri_ws/src /ros4hri_ws/src
RUN bash -lc 'source /opt/ros/jazzy/setup.bash && cd /ros4hri_ws && colcon build --merge-install'
RUN echo 'source /ros4hri_ws/install/setup.bash' >> /root/.bashrc
ENV RMW_IMPLEMENTATION=rmw_zenoh_cpp
CMD ["bash"]
