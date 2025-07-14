import React, { useEffect, useRef, useState, useCallback } from 'react';
import {
  Chart as ChartJS,
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  BarElement,
  Title,
  Tooltip,
  Legend,
} from 'chart.js';
import { Line, Bar, Scatter } from 'react-chartjs-2';

ChartJS.register(
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  BarElement,
  Title,
  Tooltip,
  Legend
);

// Static colors array to avoid re-creation on each render
const CHART_COLORS = [
  '#FF6384', '#36A2EB', '#FFCE56', '#4BC0C0',
  '#9966FF', '#FF9F40', '#E74C3C', '#2ECC71'
];

const TimeSeriesChart = ({ 
  historyData, 
  selectedKeys, 
  chartType = 'line', 
  timeRange = 'all'
}) => {
  const chartRef = useRef(null);
  const [chartData, setChartData] = useState({ datasets: [] });

  const prepareChartData = useCallback(() => {
    if (!historyData || selectedKeys.size === 0) {
      setChartData({ datasets: [] });
      return;
    }

    const datasets = [];
    let colorIndex = 0;

    for (const [playerId, playerData] of Object.entries(historyData)) {
        for (const [trackName, trackData] of Object.entries(playerData)) {
            if (selectedKeys.has(trackName) && trackData.length > 0) {
                let data = trackData;
                
                // Apply time range filter
                if (timeRange !== 'all') {
                  const limit = parseInt(timeRange.replace('last', ''));
                  data = data.slice(-limit);
                }
        
                const baseColor = CHART_COLORS[colorIndex % CHART_COLORS.length];
        
                // Create original data dataset
                const dataset = {
                  label: `${playerId} - ${trackName}`,
                  data: data.map(([timestamp, value]) => ({ x: timestamp, y: value })),
                  borderColor: baseColor,
                  backgroundColor: baseColor + '20',
                  tension: chartType === 'line' ? 0.4 : 0,
                  fill: false,
                  pointRadius: chartType === 'scatter' ? 4 : 2,
                  pointHoverRadius: 6,
                  borderWidth: 2
                };
                
                datasets.push(dataset);
                colorIndex++;
            }
        }
    }

    setChartData({ datasets });
  }, [historyData, selectedKeys, chartType, timeRange]);


  useEffect(() => {
    prepareChartData();
  }, [prepareChartData]);

  const chartOptions = {
    responsive: true,
    maintainAspectRatio: true,
    interaction: {
      mode: 'index',
      intersect: false
    },
    plugins: {
      title: {
        display: true,
        text: 'Animation Time Series Data',
        font: { size: 16 }
      },
      legend: {
        display: true,
        position: 'top'
      },
      tooltip: {
        callbacks: {
          title: function(context) {
            return `Time: ${new Date(context[0].parsed.x).toLocaleTimeString()}`;
          },
          label: function(context) {
            const value = context.parsed.y.toFixed(3);
            return `${context.dataset.label.slice(-8,-1)}: ${value}`;
          }
        }
      }
    },
    scales: {
      x: {
        type: 'linear',
        display: true,
        title: {
          display: true,
          text: 'Time'
        },
        // grid: {
        //   color: '#e0e0e0'
        // },
        bounds: "data",
        ticks: {
          display: false,
          // maxRotation: 0,
          // major: {
          //   enabled: false
          // }
        }
      },
      y: {
        type: 'linear',
        display: true,
        position: 'left',
        title: {
          display: true,
          text: 'Value'
        },
        grid: {
          color: '#e0e0e0'
        }
      },
    },
    elements: {
      point: {
        radius: chartType === 'scatter' ? 4 : 2
      }
    },
    animation: false
  };

  const renderChart = () => {
    switch (chartType) {
      case 'bar':
        return <Bar ref={chartRef} data={chartData} options={chartOptions} />;
      case 'scatter':
        return <Scatter ref={chartRef} data={chartData} options={chartOptions} />;
      case 'line':
      default:
        return <Line ref={chartRef} data={chartData} options={chartOptions} />;
    }
  };

  return (
    <div className="chart-container">
      <div className="chart-header">
        <h3>Time Series Chart</h3>
      </div>
      <div className="chart-wrapper">
        {chartData.datasets.length === 0 ? (
          <div className="no-chart-data">
            No data to display. Select keys and ensure history is being captured.
          </div>
        ) : (
          renderChart()
        )}
      </div>
    </div>
  );
};

export default TimeSeriesChart;
