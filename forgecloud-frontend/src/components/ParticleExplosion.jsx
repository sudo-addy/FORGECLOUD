"use client";
import { useEffect, useRef } from 'react';

export default function ParticleExplosion() {
    const canvasRef = useRef(null);

    useEffect(() => {
        const canvas = canvasRef.current;
        if (!canvas) return;
        const ctx = canvas.getContext('2d', { willReadFrequently: true });
        
        let animationFrameId;
        let particlesArray = [];
        let particleSize = 2;
        let resolution = 8;
        let mouse = {
            x: null,
            y: null,
            radius: 150
        };

        const handleMouseMove = (event) => {
            mouse.x = event.clientX;
            mouse.y = event.clientY;
        };

        const handleMouseLeave = () => {
            mouse.x = null;
            mouse.y = null;
        };

        window.addEventListener('mousemove', handleMouseMove);
        window.addEventListener('mouseleave', handleMouseLeave);

        const resizeCanvas = () => {
            canvas.width = window.innerWidth;
            canvas.height = window.innerHeight;
            if (image.complete) init();
        };

        window.addEventListener('resize', resizeCanvas);
        resizeCanvas();

        const image = new window.Image();
        image.src = '/datacube.png';

        class Particle {
            constructor(x, y, color) {
                this.x = x + (Math.random() * 800 - 400); 
                this.y = y + (Math.random() * 800 - 400);
                this.baseX = x;
                this.baseY = y;
                this.color = color;
                this.size = particleSize;
                this.density = (Math.random() * 30) + 5;
                this.z = Math.random() * 2 + 0.5;
            }

            draw() {
                ctx.fillStyle = this.color;
                ctx.beginPath();
                ctx.arc(this.x, this.y, this.size * this.z, 0, Math.PI * 2);
                ctx.closePath();
                ctx.fill();
            }

            update() {
                let dx = mouse.x - this.x;
                let dy = mouse.y - this.y;
                let distance = Math.sqrt(dx * dx + dy * dy);
                let forceDirectionX = dx / distance;
                let forceDirectionY = dy / distance;
                let maxDistance = mouse.radius;
                let force = (maxDistance - distance) / maxDistance;
                let directionX = forceDirectionX * force * this.density;
                let directionY = forceDirectionY * force * this.density;

                if (distance < maxDistance) {
                    this.x -= directionX;
                    this.y -= directionY;
                } else {
                    if (this.x !== this.baseX) {
                        let dx = this.x - this.baseX;
                        this.x -= dx / 15;
                    }
                    if (this.y !== this.baseY) {
                        let dy = this.y - this.baseY;
                        this.y -= dy / 15;
                    }
                }
            }
        }

        function init() {
            particlesArray = [];
            
            const imgRatio = image.width / image.height;
            let drawHeight = canvas.height * 0.6;
            let drawWidth = drawHeight * imgRatio;
            
            if (drawWidth > canvas.width * 0.6) {
                drawWidth = canvas.width * 0.6;
                drawHeight = drawWidth / imgRatio;
            }
            
            const startX = (canvas.width / 2) - (drawWidth / 2);
            const startY = (canvas.height / 2) - (drawHeight / 2) - 50;

            const offCanvas = document.createElement('canvas');
            const offCtx = offCanvas.getContext('2d', { willReadFrequently: true });
            offCanvas.width = canvas.width;
            offCanvas.height = canvas.height;
            
            offCtx.drawImage(image, startX, startY, drawWidth, drawHeight);
            const imageData = offCtx.getImageData(0, 0, offCanvas.width, offCanvas.height);
            const data = imageData.data;

            for (let y = 0; y < canvas.height; y += resolution) {
                for (let x = 0; x < canvas.width; x += resolution) {
                    const index = (y * canvas.width + x) * 4;
                    const r = data[index];
                    const g = data[index + 1];
                    const b = data[index + 2];
                    const alpha = data[index + 3];
                    
                    if (alpha > 128 && (r + g + b > 30)) {
                        const color = `rgba(${Math.min(r+30, 255)},${Math.min(g+30, 255)},${Math.min(b+50, 255)},${alpha/255})`;
                        particlesArray.push(new Particle(x, y, color));
                    }
                }
            }
        }

        function animate() {
            animationFrameId = requestAnimationFrame(animate);
            ctx.clearRect(0, 0, canvas.width, canvas.height);
            
            for (let i = 0; i < particlesArray.length; i++) {
                particlesArray[i].draw();
                particlesArray[i].update();
            }
        }

        image.onload = function() {
            init();
            animate();
        };

        return () => {
            window.removeEventListener('mousemove', handleMouseMove);
            window.removeEventListener('mouseleave', handleMouseLeave);
            window.removeEventListener('resize', resizeCanvas);
            cancelAnimationFrame(animationFrameId);
        };
    }, []);

    return (
        <canvas
            ref={canvasRef}
            className="absolute inset-0 z-0 pointer-events-none"
            style={{ width: '100%', height: '100%' }}
        />
    );
}
